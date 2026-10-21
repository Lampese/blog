//! Registered ranges using native, nonexpanding Loro marks.
//!
//! The copied reference is stable. Each range has its own attribute key, so
//! overlapping ranges do not overwrite one another. The generator uses the
//! rendered content projection as this document's text.

use crate::{AnchorError, Scalar, TextRange, Utf16, typst_string};
use loro::{
    ExpandType, ExportMode, LoroDoc, LoroValue, StyleConfig, StyleConfigMap, TextDelta,
    ValueOrContainer,
};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use thiserror::Error;

const SCHEMA: u32 = 1;
const STYLE: &str = "reference";

#[derive(Debug, Error)]
pub enum ReferenceError {
    #[error(transparent)]
    Position(#[from] AnchorError),
    #[error(transparent)]
    Loro(#[from] loro::LoroError),
    #[error(transparent)]
    Export(#[from] loro::LoroEncodeError),
    #[error(transparent)]
    Update(#[from] loro::UpdateTimeoutError),
    #[error("unsupported reference schema {0}")]
    UnsupportedSchema(u32),
    #[error("reference belongs to another article")]
    WrongArticle,
    #[error("documents do not share the same article history")]
    UnrelatedHistory,
    #[error("merged updates depend on history that is not available")]
    MissingHistory,
    #[error("range {0} is not registered")]
    UnknownRange(String),
    #[error("referenced content in range {0} was deleted; the reference is invalid")]
    DeletedRange(String),
    #[error("range {0} is no longer contiguous")]
    FragmentedRange(String),
    #[error("invalid reference state: {0}")]
    InvalidState(String),
}

/// Copied metadata; it contains no mutable offsets or cursors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeReference {
    pub schema: u32,
    pub article: String,
    pub range: String,
}

impl RangeReference {
    pub fn to_typst_metadata(&self) -> String {
        format!(
            "#metadata((schema: {}, article: {}, range: {})) <blog-reference>\n",
            self.schema,
            typst_string(&self.article),
            typst_string(&self.range),
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Selection {
    pub range: TextRange<Scalar>,
    pub text: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RangeState {
    Valid(Selection),
    Deleted,
    Fragmented,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RegisteredRange {
    pub reference: RangeReference,
    pub state: RangeState,
}

/// A single article's source text and registered ranges in the same CRDT state.
#[derive(Debug)]
pub struct RangeDocument {
    doc: LoroDoc,
    article: String,
    lineage: String,
}

impl RangeDocument {
    pub fn new(article: impl Into<String>, source: &str) -> Result<Self, ReferenceError> {
        let article = article.into();
        if article.trim().is_empty() {
            return Err(ReferenceError::InvalidState("article ID is empty".into()));
        }
        let doc = LoroDoc::new();
        configure(&doc);
        let lineage = format!("{:016x}", doc.peer_id());
        let metadata = doc.get_map("metadata");
        metadata.insert("schema", i64::from(SCHEMA))?;
        metadata.insert("article", article.as_str())?;
        metadata.insert("lineage", lineage.as_str())?;
        doc.get_text("source").insert(0, source)?;
        doc.commit();
        Ok(Self {
            doc,
            article,
            lineage,
        })
    }

    pub fn from_snapshot(bytes: &[u8]) -> Result<Self, ReferenceError> {
        let doc = LoroDoc::from_snapshot(bytes)?;
        configure(&doc);
        if !matches!(
            doc.get_map("metadata").get("schema"),
            Some(ValueOrContainer::Value(LoroValue::I64(value))) if value == i64::from(SCHEMA)
        ) {
            return Err(ReferenceError::InvalidState(
                "unsupported document schema".into(),
            ));
        }
        let article = metadata_string(&doc, "article")?;
        let lineage = metadata_string(&doc, "lineage")?;
        if article.trim().is_empty() || lineage.is_empty() {
            return Err(ReferenceError::InvalidState(
                "missing article identity".into(),
            ));
        }
        Ok(Self {
            doc,
            article,
            lineage,
        })
    }

    pub fn source(&self) -> String {
        self.doc.get_text("source").to_string()
    }

    pub fn article_id(&self) -> &str {
        &self.article
    }

    pub fn range_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        self.doc
            .get_map("ranges")
            .for_each(|id, _| ids.push(id.to_owned()));
        ids.sort();
        ids
    }

    /// Known invalid ranges remain queryable; corrupt state remains an error.
    pub fn references(&self) -> impl Iterator<Item = Result<RegisteredRange, ReferenceError>> + '_ {
        self.range_ids().into_iter().map(|id| {
            let reference = RangeReference {
                schema: SCHEMA,
                article: self.article.clone(),
                range: id,
            };
            let state = match self.resolve(&reference) {
                Ok(selection) => RangeState::Valid(selection),
                Err(ReferenceError::DeletedRange(_)) => RangeState::Deleted,
                Err(ReferenceError::FragmentedRange(_)) => RangeState::Fragmented,
                Err(error) => return Err(error),
            };
            Ok(RegisteredRange { reference, state })
        })
    }

    /// Adopt a selection made against a published snapshot. Rebase its marks
    /// through the latest rendered text; existing registered ranges stay intact.
    pub fn adopt_selection(
        &mut self,
        reference: &RangeReference,
        seed: &[u8],
        utf16_range: TextRange<Utf16>,
    ) -> Result<(), ReferenceError> {
        if reference.schema != SCHEMA {
            return Err(ReferenceError::UnsupportedSchema(reference.schema));
        }
        if reference.article != self.article {
            return Err(ReferenceError::WrongArticle);
        }
        if self.doc.get_map("ranges").get(&reference.range).is_some() {
            self.resolve(reference)?;
            return Ok(());
        }
        let mut basis = Self::from_snapshot(seed)?;
        if basis.article != self.article || basis.lineage != self.lineage {
            return Err(ReferenceError::UnrelatedHistory);
        }
        let source = basis.source();
        let range = utf16_range.convert::<Scalar>(&source)?;
        basis.register_named(&reference.range, range)?;
        basis.update_source(&self.source())?;
        let current = basis.resolve(reference)?;
        self.register_named(&reference.range, current.range)?;
        Ok(())
    }

    fn register_named(&mut self, id: &str, range: TextRange<Scalar>) -> Result<(), ReferenceError> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        {
            return Err(ReferenceError::InvalidState("invalid range ID".into()));
        }
        let text = self.doc.get_text("source");
        if range.is_empty() || range.end() > text.len_unicode() {
            return Err(AnchorError::InvalidRange.into());
        }
        text.mark(range.offsets(), &style_key(id), true)?;
        self.doc.get_map("ranges").insert(id, true)?;
        self.doc.commit();
        Ok(())
    }

    pub fn register(&mut self, range: TextRange<Scalar>) -> Result<RangeReference, ReferenceError> {
        let text = self.doc.get_text("source");
        if range.is_empty() || range.end() > text.len_unicode() {
            return Err(AnchorError::InvalidRange.into());
        }
        self.doc.commit();
        let peer = self.doc.peer_id();
        let counter = self.doc.oplog_vv().get(&peer).copied().unwrap_or(0);
        let id = format!("r-{peer:016x}-{counter:x}");
        text.mark(range.offsets(), &style_key(&id), true)?;
        self.doc.get_map("ranges").insert(&id, true)?;
        self.doc.commit();
        Ok(RangeReference {
            schema: SCHEMA,
            article: self.article.clone(),
            range: id,
        })
    }

    /// Input is a source-text UTF-16 range, not yet a rendered DOM selection.
    pub fn register_utf16(
        &mut self,
        range: TextRange<Utf16>,
    ) -> Result<RangeReference, ReferenceError> {
        let source = self.source();
        self.register(range.convert::<Scalar>(&source)?)
    }

    pub fn resolve(&self, reference: &RangeReference) -> Result<Selection, ReferenceError> {
        if reference.schema != SCHEMA {
            return Err(ReferenceError::UnsupportedSchema(reference.schema));
        }
        if reference.article != self.article {
            return Err(ReferenceError::WrongArticle);
        }
        match self.doc.get_map("ranges").get(&reference.range) {
            Some(ValueOrContainer::Value(LoroValue::Bool(true))) => {}
            None => return Err(ReferenceError::UnknownRange(reference.range.clone())),
            Some(_) => {
                return Err(ReferenceError::InvalidState(
                    "invalid range registration".into(),
                ));
            }
        }
        let key = style_key(&reference.range);
        let mut offset = 0;
        let mut spans: Vec<Range<usize>> = Vec::new();
        let mut selected = String::new();
        for delta in self.doc.get_text("source").to_delta() {
            let TextDelta::Insert { insert, attributes } = delta else {
                return Err(ReferenceError::InvalidState(
                    "unexpected document delta".into(),
                ));
            };
            let end = offset + insert.chars().count();
            if attributes.is_some_and(|attrs| attrs.get(&key) == Some(&LoroValue::Bool(true))) {
                if let Some(previous) = spans.last_mut()
                    && previous.end == offset
                {
                    previous.end = end;
                } else {
                    spans.push(offset..end);
                }
                selected.push_str(&insert);
            }
            offset = end;
        }
        match spans.as_slice() {
            [] => Err(ReferenceError::DeletedRange(reference.range.clone())),
            [span] => Ok(Selection {
                range: TextRange::new(span.clone()),
                text: selected,
            }),
            _ => Err(ReferenceError::FragmentedRange(reference.range.clone())),
        }
    }

    /// Reconcile an ordinary editor's saved source using Loro's text diff.
    /// Stage changes in a fork so a diff failure cannot leave partial edits.
    pub fn update_source(&mut self, source: &str) -> Result<(), ReferenceError> {
        if self.source() == source {
            return Ok(());
        }
        let staged = self.doc.fork();
        configure(&staged);
        staged
            .get_text("source")
            .update(source, Default::default())?;
        staged.commit();
        self.doc = staged;
        Ok(())
    }

    pub fn fork(&self) -> Self {
        let doc = self.doc.fork();
        configure(&doc);
        Self {
            doc,
            article: self.article.clone(),
            lineage: self.lineage.clone(),
        }
    }

    /// Reconcile registered ranges after both snapshots have been projected onto
    /// the same resolved article. Observed deletions win; divergent positions fail.
    /// This does not reconstruct or merge discarded operation history.
    pub fn reconcile_ranges(&mut self, other: &Self) -> Result<(), ReferenceError> {
        if self.article != other.article || self.lineage != other.lineage {
            return Err(ReferenceError::UnrelatedHistory);
        }
        if self.source() != other.source() {
            return Err(ReferenceError::InvalidState(
                "range reconciliation requires the same rendered article".into(),
            ));
        }
        let mut staged = self.fork();
        let existing = staged.range_ids();
        for id in other.range_ids() {
            let reference = RangeReference {
                schema: SCHEMA,
                article: self.article.clone(),
                range: id.clone(),
            };
            match other.resolve(&reference) {
                Err(ReferenceError::DeletedRange(_)) => {
                    let text = staged.doc.get_text("source");
                    if text.len_unicode() > 0 {
                        text.unmark(0..text.len_unicode(), &style_key(&id))?;
                    }
                    staged.doc.get_map("ranges").insert(&id, true)?;
                    staged.doc.commit();
                }
                Ok(selection) if existing.contains(&id) => match staged.resolve(&reference) {
                    Ok(current) if current != selection => {
                        return Err(ReferenceError::InvalidState(format!(
                            "range {id} resolves to different positions across snapshots; select a new reference explicitly"
                        )));
                    }
                    Ok(_) | Err(ReferenceError::DeletedRange(_)) => {}
                    Err(error) => return Err(error),
                },
                Ok(selection) => staged.register_named(&id, selection.range)?,
                Err(error) => return Err(error),
            }
        }
        *self = staged;
        Ok(())
    }

    /// Merge a same-origin branch. Old branches beyond a compaction frontier
    /// are not automatically reconstructed; Loro import errors are propagated.
    pub fn merge(&mut self, other: &Self) -> Result<(), ReferenceError> {
        if self.article != other.article || self.lineage != other.lineage {
            return Err(ReferenceError::UnrelatedHistory);
        }
        let staged = self.doc.fork();
        configure(&staged);
        let imported = staged.import(&other.snapshot()?)?;
        if imported.pending.is_some() {
            return Err(ReferenceError::MissingHistory);
        }
        staged.commit();
        self.doc = staged;
        Ok(())
    }

    /// Full history, useful while branches may still depend on old operations.
    pub fn snapshot(&self) -> Result<Vec<u8>, ReferenceError> {
        Ok(self.doc.export(ExportMode::Snapshot)?)
    }

    /// Latest state with minimal history. This does not promise that arbitrary
    /// older branches can still be merged after this compaction boundary.
    pub fn state_snapshot(&self) -> Result<Vec<u8>, ReferenceError> {
        self.doc.commit();
        Ok(self.doc.export(ExportMode::StateOnly(None))?)
    }
}

fn configure(doc: &LoroDoc) {
    let mut styles = StyleConfigMap::new();
    styles.insert(
        STYLE.into(),
        StyleConfig {
            expand: ExpandType::None,
        },
    );
    doc.config_text_style(styles);
}

fn style_key(id: &str) -> String {
    format!("{STYLE}:{id}")
}

fn metadata_string(doc: &LoroDoc, key: &str) -> Result<String, ReferenceError> {
    match doc.get_map("metadata").get(key) {
        Some(ValueOrContainer::Value(LoroValue::String(value))) => Ok(value.to_string()),
        _ => Err(ReferenceError::InvalidState(format!(
            "missing string field {key}"
        ))),
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[test]
    fn querying_registered_ranges_distinguishes_deletion_from_corrupt_state() {
        let mut document = RangeDocument::new("a", "abcdef").unwrap();
        let deleted = document.register(TextRange::new(0..2)).unwrap();
        let valid = document.register(TextRange::new(4..6)).unwrap();
        document.update_source("cdef").unwrap();
        let ranges = document
            .references()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            ranges
                .iter()
                .any(|entry| entry.reference == deleted && entry.state == RangeState::Deleted)
        );
        assert!(ranges.iter().any(|entry| entry.reference == valid
            && matches!(&entry.state, RangeState::Valid(selection) if selection.text == "ef")));
        document
            .doc
            .get_map("ranges")
            .insert(&valid.range, "invalid registration")
            .unwrap();
        assert!(
            document
                .references()
                .any(|entry| matches!(entry, Err(ReferenceError::InvalidState(_))))
        );
    }

    #[test]
    fn compacted_branches_reconcile_new_ranges_and_preserve_deletions() {
        let mut basis = RangeDocument::new("a", "AabcdefB").unwrap();
        let first = basis.register(TextRange::new(1..5)).unwrap();
        let mut branch = basis.fork();
        let second = branch.register(TextRange::new(3..7)).unwrap();
        branch.update_source("AefB").unwrap();
        branch.update_source("AabcdefB").unwrap();
        let mut restored = RangeDocument::from_snapshot(&basis.state_snapshot().unwrap()).unwrap();
        let branch = RangeDocument::from_snapshot(&branch.state_snapshot().unwrap()).unwrap();
        restored.reconcile_ranges(&branch).unwrap();
        assert!(matches!(
            restored.resolve(&first),
            Err(ReferenceError::DeletedRange(_))
        ));
        assert_eq!(restored.resolve(&second).unwrap().text, "ef");
        restored.update_source("prefix AabcdefB suffix").unwrap();
        assert!(matches!(
            restored.resolve(&first),
            Err(ReferenceError::DeletedRange(_))
        ));
    }

    #[test]
    fn divergent_positions_fail_without_mutating_the_selected_basis() {
        let mut basis = RangeDocument::new("a", "abab").unwrap();
        let reference = basis.register(TextRange::new(0..2)).unwrap();
        let branch = basis.fork();
        let text = branch.doc.get_text("source");
        text.unmark(0..4, &style_key(&reference.range)).unwrap();
        text.mark(2..4, &style_key(&reference.range), true).unwrap();
        branch.doc.commit();
        assert!(basis.reconcile_ranges(&branch).is_err());
        assert_eq!(
            basis.resolve(&reference).unwrap().range,
            TextRange::new(0..2)
        );
        assert_eq!(basis.source(), "abab");
    }
}
