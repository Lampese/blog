//! Inspect and recover the managed state without hand-editing encoded snapshots.
use crate::{content, storage::atomic_write};
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use blog_ranges::{RangeDocument, RangeReference, ReferenceError};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use typst_syntax::ast::{self, Arg, Expr};

#[derive(Serialize)]
pub struct RangeStatus {
    pub id: String,
    pub status: &'static str,
    pub preview: Option<String>,
}

#[derive(Serialize)]
pub struct SnapshotReport {
    pub article: String,
    pub characters: usize,
    pub ranges: Vec<RangeStatus>,
}

#[derive(Serialize)]
pub struct Inspection {
    pub file: PathBuf,
    pub conflicts: bool,
    pub snapshots: Vec<SnapshotReport>,
}

#[derive(Serialize)]
pub struct RecoveryReport {
    pub file: PathBuf,
    pub written: bool,
    pub backup: Option<PathBuf>,
    pub snapshots_reconciled: usize,
    pub state: SnapshotReport,
    pub warnings: Vec<String>,
}

pub fn inspect(path: &Path) -> Result<Inspection> {
    let raw = fs::read_to_string(path)?;
    let (_, snapshots) = parse(&raw)?;
    Ok(Inspection {
        file: path.to_owned(),
        conflicts: has_conflicts(&raw),
        snapshots: snapshots
            .iter()
            .map(|bytes| describe(&RangeDocument::from_snapshot(bytes)?))
            .collect::<Result<_>>()?,
    })
}

pub(crate) fn describe(state: &RangeDocument) -> Result<SnapshotReport> {
    let mut ranges = Vec::new();
    for id in state.range_ids() {
        let reference = RangeReference {
            schema: 1,
            article: state.article_id().to_owned(),
            range: id.clone(),
        };
        let (status, preview) = match state.resolve(&reference) {
            Ok(selection) => ("valid", Some(selection.text.chars().take(96).collect())),
            Err(ReferenceError::DeletedRange(_)) => ("deleted", None),
            Err(ReferenceError::FragmentedRange(_)) => ("fragmented", None),
            Err(error) => return Err(error.into()),
        };
        ranges.push(RangeStatus {
            id,
            status,
            preview,
        });
    }
    Ok(SnapshotReport {
        article: state.article_id().to_owned(),
        characters: state.source().chars().count(),
        ranges,
    })
}

pub(crate) fn conflict_marker(line: &str) -> bool {
    ["<<<<<<<", "|||||||", "=======", ">>>>>>>"]
        .iter()
        .any(|marker| line.starts_with(marker))
}

fn has_conflicts(source: &str) -> bool {
    use typst_syntax::{SyntaxKind, SyntaxNode};
    fn literals(node: &SyntaxNode, offset: usize, ranges: &mut Vec<std::ops::Range<usize>>) {
        if matches!(
            node.kind(),
            SyntaxKind::Raw | SyntaxKind::Str | SyntaxKind::LineComment | SyntaxKind::BlockComment
        ) {
            ranges.push(offset..offset + node.len());
            return;
        }
        let mut next = offset;
        for child in node.children() {
            literals(child, next, ranges);
            next += child.len();
        }
    }
    let mut ignored = Vec::new();
    literals(&typst_syntax::parse(source), 0, &mut ignored);
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        if conflict_marker(line) && !ignored.iter().any(|range| range.contains(&offset)) {
            return true;
        }
        offset += line.len();
    }
    false
}

pub(crate) fn article_body(raw: &str) -> Result<&str> {
    let body = raw
        .split_once(content::STATE_START)
        .map_or(raw, |(body, _)| body);
    if has_conflicts(body) {
        bail!("Resolve the article's source conflicts before recovering its managed state");
    }
    Ok(body.trim_end())
}

/// Only decode the literal, generated footer. Article metadata stays with Typst.
pub(crate) fn parse(raw: &str) -> Result<(&str, Vec<Vec<u8>>)> {
    let Some((body, footer)) = raw.split_once(content::STATE_START) else {
        return Ok((raw.trim_end(), Vec::new()));
    };
    let mut snapshots = Vec::new();
    let mut closed = false;
    for line in footer.lines().map(str::trim) {
        if line.is_empty() || conflict_marker(line) || line == content::STATE_START {
            continue;
        }
        if line == content::STATE_END {
            closed = true;
            continue;
        }
        let literal = line
            .strip_prefix("#metadata(")
            .and_then(|line| line.strip_suffix(") <blog-state>"))
            .context("Malformed managed state; use --from with a known-good article snapshot")?;
        let tree = typst_syntax::parse(&format!("#metadata({literal})"));
        fn argument(node: &typst_syntax::SyntaxNode) -> Option<String> {
            if let Some(call) = node.cast::<ast::FuncCall>() {
                if !matches!(call.callee(), Expr::Ident(name) if name.get() == "metadata") {
                    return None;
                }
                let mut args = call.args().items();
                let Arg::Pos(Expr::Str(value)) = args.next()? else {
                    return None;
                };
                return args.next().is_none().then(|| value.get().to_string());
            }
            node.children().find_map(argument)
        }
        if !tree.errors_and_warnings().0.is_empty() {
            bail!("Malformed state literal");
        }
        let encoded =
            argument(&tree).context("Managed state must contain a literal snapshot string")?;
        let bytes = STANDARD
            .decode(encoded)
            .context("Invalid base64 in managed state")?;
        RangeDocument::from_snapshot(&bytes).context("Invalid managed state snapshot")?;
        if !snapshots.contains(&bytes) {
            snapshots.push(bytes);
        }
    }
    if !closed || snapshots.is_empty() {
        bail!("Incomplete managed state footer; use --from with a known-good article snapshot");
    }
    Ok((body.trim_end(), snapshots))
}

/// A recovery checks the entire site under the build lock. The original is
/// backed up before staging; failures and unwinding restore its exact bytes.
pub(crate) struct SourceEdit {
    path: PathBuf,
    original: String,
    pub backup: PathBuf,
    finished: bool,
    staged: Option<String>,
}

impl SourceEdit {
    pub fn new(root: &Path, path: &Path, original: String) -> Result<Self> {
        let directory = root.join(".blog/state-backups");
        fs::create_dir_all(&directory)?;
        let backup = tempfile::Builder::new()
            .prefix("article-")
            .suffix(".typ")
            .tempfile_in(directory)?;
        fs::write(backup.path(), &original)?;
        backup.as_file().sync_all()?;
        let (_, backup) = backup.keep()?;
        Ok(Self {
            path: path.to_owned(),
            original,
            backup,
            finished: false,
            staged: None,
        })
    }
    pub fn stage(&mut self, source: &str) -> Result<()> {
        if fs::read_to_string(&self.path)? != self.original {
            bail!(
                "The article changed before recovery and was left untouched; original source preserved at {}",
                self.backup.display()
            );
        }
        atomic_write(&self.path, source.as_bytes())?;
        self.staged = Some(source.to_owned());
        Ok(())
    }
    pub fn finish(mut self, source: &str, write: bool) -> Result<Option<PathBuf>> {
        if self.staged.as_deref() != Some(fs::read_to_string(&self.path)?.as_str()) {
            self.finished = true;
            bail!(
                "The article changed during recovery and was left untouched; the original is preserved at {}",
                self.backup.display()
            );
        }
        atomic_write(
            &self.path,
            if write {
                source.as_bytes()
            } else {
                self.original.as_bytes()
            },
        )?;
        self.finished = true;
        if write {
            Ok(Some(self.backup.clone()))
        } else {
            fs::remove_file(&self.backup)?;
            Ok(None)
        }
    }
}

impl Drop for SourceEdit {
    fn drop(&mut self) {
        if !self.finished
            && self.staged.is_some()
            && fs::read_to_string(&self.path).ok().as_deref() != self.staged.as_deref()
        {
            eprintln!(
                "The article changed during recovery and was left untouched; original source preserved at {}",
                self.backup.display()
            );
            return;
        }
        if !self.finished
            && self.staged.is_some()
            && let Err(error) = atomic_write(&self.path, self.original.as_bytes())
        {
            eprintln!(
                "Restoring {} failed: {error}; original source preserved at {}",
                self.path.display(),
                self.backup.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_conflict_examples_in_raw_blocks_are_article_content() {
        let example =
            "A Git example:\n```text\n<<<<<<< ours\nalpha\n=======\nbeta\n>>>>>>> theirs\n```\n";
        assert!(!has_conflicts(example));
        assert!(article_body(example).is_ok());
        assert!(has_conflicts(
            "<<<<<<< ours\nalpha\n=======\nbeta\n>>>>>>> theirs\n"
        ));
    }

    #[test]
    fn recovery_does_not_overwrite_an_edit_made_during_validation() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("article.typ");
        fs::write(&path, "original")?;
        let mut edit = SourceEdit::new(root.path(), &path, "original".to_owned())?;
        edit.stage("staged recovery")?;
        let backup = edit.backup.clone();
        fs::write(&path, "concurrent editor change")?;
        assert!(edit.finish("recovered state", true).is_err());
        assert_eq!(fs::read_to_string(path)?, "concurrent editor change");
        assert_eq!(fs::read_to_string(backup)?, "original");
        Ok(())
    }

    #[test]
    fn recovery_does_not_stage_over_a_newer_editor_save() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("article.typ");
        fs::write(&path, "original")?;
        let mut edit = SourceEdit::new(root.path(), &path, "original".to_owned())?;
        fs::write(&path, "newer editor save")?;
        assert!(edit.stage("staged recovery").is_err());
        drop(edit);
        assert_eq!(fs::read_to_string(path)?, "newer editor save");
        Ok(())
    }
}
