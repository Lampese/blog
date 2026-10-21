//! Resolve transclusion dependencies before assigning article-local identities.
use super::BuiltPost;
use crate::{
    content::{self, Include, Page, RawPost, TypstCompiler},
    dom::{Compiled, Expanded},
    locale::Locale,
};
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use blog_ranges::RangeDocument;
use std::{collections::BTreeMap, path::Path};

pub(super) struct ArticleGraph {
    sources: BTreeMap<String, RawPost>,
    pub built: BTreeMap<String, BuiltPost>,
    visiting: Vec<String>,
    canonical: Locale,
    recovery: Option<(std::path::PathBuf, Vec<Vec<u8>>)>,
}

/// A failed evaluation still owns the range states updated before the failure.
pub(super) struct Evaluation {
    articles: ArticleGraph,
    about: Result<Option<Page<Expanded>>>,
}

pub(super) struct ResolvedContent {
    pub posts: BTreeMap<String, BuiltPost>,
    pub about: Option<Page<Expanded>>,
}

impl Evaluation {
    pub fn save_observed_states(&self) -> Result<()> {
        self.articles
            .built
            .values()
            .try_for_each(|post| content::save_state(&post.path, &post.state))
    }

    pub fn validate(self) -> Result<ResolvedContent> {
        Ok(ResolvedContent {
            about: self.about?,
            posts: self.articles.built,
        })
    }
}

impl ResolvedContent {
    pub fn compact_receipts(&self, root: &Path, about: Option<&str>) -> Result<()> {
        let about = about.map(|path| root.join(path));
        for path in self
            .posts
            .values()
            .map(|post| post.path.as_path())
            .chain(about.as_deref())
        {
            content::compact_receipts(path, |reference| {
                self.posts
                    .get(&reference.article)
                    .is_some_and(|post| post.state.resolve(reference).is_ok())
            })?;
        }
        Ok(())
    }
}

impl ArticleGraph {
    pub fn load(
        root: &Path,
        content: &str,
        compiler: &TypstCompiler<'_>,
        drafts: bool,
        canonical: &Locale,
    ) -> Result<Self> {
        let mut sources = BTreeMap::new();
        for entry in walkdir::WalkDir::new(root.join(content)).sort_by_file_name() {
            let entry = entry?;
            if entry.file_type().is_symlink() {
                bail!(
                    "Symbolic links are not supported in the content directory: {}",
                    entry.path().display()
                );
            }
            if entry.file_type().is_file() && entry.path().extension().is_some_and(|e| e == "typ") {
                let post = compiler
                    .compile(entry.path())
                    .with_context(|| format!("Loading article {}", entry.path().display()))?;
                if post.meta.draft && !drafts {
                    continue;
                }
                let id = post.meta.id.clone();
                if sources.insert(id.clone(), post).is_some() {
                    bail!("Duplicate article ID: {id}");
                }
            }
        }
        Ok(Self {
            sources,
            built: BTreeMap::new(),
            visiting: Vec::new(),
            canonical: canonical.clone(),
            recovery: None,
        })
    }

    pub fn recover(&mut self, path: &Path, snapshots: Vec<Vec<u8>>) {
        self.recovery = Some((path.to_owned(), snapshots));
    }

    pub fn evaluate(mut self, about: Option<Page<Compiled>>) -> Evaluation {
        let ids: Vec<_> = self.sources.keys().cloned().collect();
        let about = ids
            .iter()
            .try_for_each(|id| self.resolve(id))
            .and_then(|()| about.map(|page| self.expand_page(page)).transpose());
        Evaluation {
            articles: self,
            about,
        }
    }

    fn expand_page(&mut self, page: Page<Compiled>) -> Result<Page<Expanded>> {
        let replacements = self.replacements("about", page.body.includes()?)?;
        Ok(Page {
            meta: page.meta,
            body: page.body.expand(&replacements)?,
        })
    }

    fn resolve(&mut self, id: &str) -> Result<()> {
        if self.built.contains_key(id) {
            return Ok(());
        }
        if self.visiting.iter().any(|entry| entry == id) {
            bail!(
                "Cyclic transclusion dependency: {} -> {id}",
                self.visiting.join(" → ")
            );
        }
        let post = self.sources.get(id).with_context(|| {
            format!("Referenced article {id} was not found (it may be a draft)")
        })?;
        let includes = post.body.includes()?;
        self.visiting.push(id.to_owned());
        let result = self.expand(id, includes);
        self.visiting.pop();
        result
    }

    fn expand(&mut self, id: &str, includes: Vec<Include>) -> Result<()> {
        let replacements = self.replacements(id, includes)?;
        let post = &self.sources[id];
        // Deployment URLs and asset versions are applied only during publication.
        let projection = post.body.expand(&replacements)?.project();
        let mut state = match &post.state {
            Some(bytes) => RangeDocument::from_snapshot(bytes)
                .with_context(|| format!("Invalid reference state in {}", post.path.display()))?,
            None => RangeDocument::new(&post.meta.id, projection.text())?,
        };
        if state.article_id() != post.meta.id {
            bail!(
                "Article ID does not match reference state in {}; preserve the ID when moving an article, or use blog new to create a new one",
                post.path.display()
            );
        }
        state.update_source(projection.text())?;
        if let Some((path, snapshots)) = &self.recovery
            && path == &post.path
        {
            for snapshot in snapshots {
                let mut branch = RangeDocument::from_snapshot(snapshot)?;
                branch.update_source(projection.text())?;
                state.reconcile_ranges(&branch)?;
            }
        }
        self.built.insert(
            id.to_owned(),
            BuiltPost {
                meta: post.meta.clone(),
                path: post.path.clone(),
                body: projection,
                state,
            },
        );
        Ok(())
    }

    fn replacements(&mut self, id: &str, includes: Vec<Include>) -> Result<Vec<String>> {
        let mut replacements = Vec::with_capacity(includes.len());
        for (index, include) in includes.into_iter().enumerate() {
            self.resolve(&include.reference.article)?;
            replacements.push(self.excerpt(id, index, include)?);
        }
        Ok(replacements)
    }

    fn excerpt(&mut self, id: &str, index: usize, include: Include) -> Result<String> {
        let reference = &include.reference;
        let target = self
            .built
            .get_mut(&reference.article)
            .context("Referenced article has not been expanded")?;
        if let Some(receipt) = include.receipt {
            if receipt.seed.len() > 12_000_000 {
                bail!("Reference receipt is too large");
            }
            let seed = STANDARD
                .decode(&receipt.seed)
                .context("Invalid reference receipt")?;
            target
                .state
                .adopt_selection(reference, &seed, receipt.range)
                .with_context(|| {
                    format!(
                        "Article {id} cannot register a selection from {}",
                        reference.article
                    )
                })?;
        }
        let selected = target.state.resolve(reference).with_context(|| {
            format!(
                "Invalid reference in article {id}: {} / {}",
                reference.article, reference.range
            )
        })?;
        let url = format!("/posts/{}/", reference.article);
        let excerpt =
            target
                .body
                .excerpt(selected.range, &format!("excerpt-{id}-{index}"), &url)?;
        let source_text = self.canonical.text("excerpt_source")?;
        let source = html_escape::encode_text(&source_text);
        let quote_open = self.canonical.text("quote_open")?;
        let quote_close = self.canonical.text("quote_close")?;
        Ok(format!(
            "<figure class=\"transclusion\"><div class=\"excerpt-content\">{excerpt}</div><figcaption><span data-blog-i18n=\"excerpt_source\">{source}</span> <a data-blog-reference=\"\" aria-haspopup=\"dialog\" href=\"{url}?range={}\"><span data-blog-i18n=\"quote_open\">{quote_open}</span>{}<span data-blog-i18n=\"quote_close\">{quote_close}</span></a></figcaption></figure>",
            reference.range,
            html_escape::encode_text(&target.meta.title)
        ))
    }
}
