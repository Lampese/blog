//! Build orchestration. Source mutations always run inside one locked session.
mod graph;

use crate::{
    config::SiteConfig,
    content::{self, Include, TypstCompiler},
    locale::Locale,
    site,
    storage::{BuildLock, Publication},
};
use anyhow::{Context, Result, bail};
use graph::ArticleGraph;
use std::{collections::BTreeMap, path::Path};

pub use crate::content::BuiltPost;

pub struct BuildResult {
    pub config: SiteConfig,
    pub posts: BTreeMap<String, BuiltPost>,
}

#[derive(Default, Clone, Copy)]
pub struct BuildOptions {
    pub drafts: bool,
    pub check: bool,
    pub preview: bool,
}

impl BuildOptions {
    pub const fn preview() -> Self {
        Self {
            drafts: true,
            check: false,
            preview: true,
        }
    }
}

pub fn build(root: &Path, options: BuildOptions) -> Result<BuildResult> {
    BuildSession::acquire(root)?.build(options)
}

pub fn register(
    root: &Path,
    result: &mut BuildResult,
    article: &str,
    revision: &str,
    start: usize,
    end: usize,
) -> Result<Include> {
    let session = BuildSession::acquire(root)?;
    *result = session.build(BuildOptions::preview())?;
    let post = result.posts.get_mut(article).context("Article not found")?;
    if post.revision != revision {
        bail!("The page has changed; refresh it and select the range again");
    }
    let reference = post.state.register_utf16(start..end)?;
    content::save_state(&post.path, &post.state)?;
    // Keep the lock across both builds so the published snapshot includes the
    // registered range, and another process cannot observe a half-finished edit.
    *result = session.build(BuildOptions::preview())?;
    Ok(Include {
        reference,
        receipt: None,
    })
}

/// Reconcile compacted reference snapshots against manually resolved article
/// source. The default checks the result; --write saves only after validation.
pub fn recover_state(
    root: &Path,
    path: &Path,
    from: &[std::path::PathBuf],
    write: bool,
) -> Result<crate::state::RecoveryReport> {
    use crate::state::{self, SourceEdit};
    use blog_ranges::RangeDocument;
    let canonical_root = root.canonicalize()?;
    let root = canonical_root.as_path();
    let session = BuildSession::acquire(root)?;
    let path = root
        .join(path)
        .canonicalize()
        .context("Article file not found")?;
    let config = SiteConfig::load(root)?;
    let content_root = root.join(&config.content).canonicalize()?;
    if !path.starts_with(content_root) {
        bail!("Recovery target must be inside the configured content directory");
    }
    let original = std::fs::read_to_string(&path)?;
    let body = state::article_body(&original)?.to_owned();
    let mut warnings = Vec::new();
    let mut snapshots = match state::parse(&original) {
        Ok((_, snapshots)) => snapshots,
        Err(error) if !from.is_empty() => {
            warnings.push(format!("The current footer could not be decoded: {error}"));
            Vec::new()
        }
        Err(error) => return Err(error),
    };
    for source in from {
        let raw = std::fs::read_to_string(root.join(source))?;
        let (_, additional) = state::parse(&raw)?;
        if additional.is_empty() {
            bail!("No managed state in {}", source.display());
        }
        snapshots.extend(additional);
    }
    let seed = snapshots
        .first()
        .context("No recoverable state; supply --from with an article snapshot")?;
    let initial = RangeDocument::from_snapshot(seed)?;
    let mut edit = SourceEdit::new(root, &path, original)?;
    edit.stage(&content::with_state(&body, &initial)?)?;
    let count = snapshots.len();
    let result = session
        .build_with_recovery(
            BuildOptions {
                drafts: true,
                check: true,
                preview: false,
            },
            Some((&path, snapshots)),
        )
        .with_context(|| {
            format!(
                "Recovery validation failed; original source preserved at {}",
                edit.backup.display()
            )
        })?;
    let post = result
        .posts
        .values()
        .find(|post| post.path == path)
        .context("Recovered article was not found")?;
    let report = state::describe(&post.state)?;
    let next = content::with_state(&body, &post.state)?;
    let backup = edit.finish(&next, write)?;
    Ok(state::RecoveryReport {
        file: path,
        written: write,
        backup,
        snapshots_reconciled: count,
        state: report,
        warnings,
    })
}

struct BuildSession<'a> {
    root: &'a Path,
    _lock: BuildLock,
}

impl<'a> BuildSession<'a> {
    fn acquire(root: &'a Path) -> Result<Self> {
        Ok(Self {
            root,
            _lock: BuildLock::acquire(root)?,
        })
    }

    fn build(&self, options: BuildOptions) -> Result<BuildResult> {
        self.build_with_recovery(options, None)
    }

    fn build_with_recovery(
        &self,
        options: BuildOptions,
        recovery: Option<(&Path, Vec<Vec<u8>>)>,
    ) -> Result<BuildResult> {
        let mut config = SiteConfig::load(self.root)?;
        Publication::validate(&self.root.join(&config.output))?;
        if options.preview {
            config.prefix.clear();
        }

        let compiler = TypstCompiler::new(self.root)?;
        let about = config
            .about
            .as_ref()
            .map(|path| compiler.compile_page(&self.root.join(path)))
            .transpose()
            .context("Loading the about page")?;
        let text = Locale::load(self.root, &config.language)?;
        config.language = text.language().to_owned();
        let canonical = Locale::canonical();
        let mut articles = ArticleGraph::load(
            self.root,
            &config.content,
            &compiler,
            options.drafts,
            &canonical,
        )?;
        if let Some((path, snapshots)) = recovery {
            articles.recover(path, snapshots);
        }
        let resolved = articles
            .resolve_all()
            .and_then(|()| about.map(|page| articles.expand_page(page)).transpose());
        // Persist observed deletions even when a dependent reference is invalid;
        // otherwise a later edit could accidentally revive a deleted range.
        if !options.check {
            articles.save_states()?;
        }
        let about = resolved?;

        if !options.check {
            articles.compact_receipts()?;
            if let Some(path) = &config.about {
                content::compact_receipts(&self.root.join(path), |reference| {
                    articles
                        .built
                        .get(&reference.article)
                        .is_some_and(|post| post.state.resolve(reference).is_ok())
                })?;
            }
            site::publish(
                self.root,
                &config,
                &articles.built,
                about.as_ref(),
                &text,
                options.preview,
            )?;
        }
        Ok(BuildResult {
            config,
            posts: articles.built,
        })
    }
}
