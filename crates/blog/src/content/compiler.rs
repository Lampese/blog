//! Typst owns evaluation and macro expansion; Rust only decodes its results.
use super::{Page, PageMeta, PostMeta, RawPost};
use crate::{
    config::valid_id,
    dom::{Compiled, Document},
    locale::Locale,
    storage::read_text_override,
};
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use blog_ranges::typst_string;
use serde::Deserialize;
use std::{fs, path::Path, process::Command};

const METADATA_QUERY: &str = "(posts: query(<blog-post>).map(it => it.value), pages: query(<blog-page>).map(it => it.value), states: query(<blog-state>).map(it => it.value))";

#[derive(Deserialize)]
struct Metadata {
    posts: Vec<PostMeta>,
    pages: Vec<PageMeta>,
    states: Vec<String>,
}

struct CompiledDocument {
    metadata: Metadata,
    body: Document<Compiled>,
}

/// The root and theme are shared by every document in a build.
pub(crate) struct TypstCompiler<'a> {
    root: &'a Path,
    root_string: &'a str,
    theme: String,
    canonical_text: String,
}

impl<'a> TypstCompiler<'a> {
    pub fn new(root: &'a Path) -> Result<Self> {
        Ok(Self {
            root,
            root_string: root.to_str().context("The project path must be UTF-8")?,
            canonical_text: format!("blog-text={}", Locale::canonical().messages_json()?),
            theme: read_text_override(
                root,
                "assets/code.tmTheme",
                include_str!("../../../../assets/code.tmTheme"),
            )?,
        })
    }

    pub fn compile(&self, path: &Path) -> Result<RawPost> {
        let document = self.compile_document(path)?;
        let metadata = document.metadata;
        if !metadata.pages.is_empty() {
            bail!(
                "Articles must not contain <blog-page> metadata: {}",
                path.display()
            );
        }
        let [meta]: [PostMeta; 1] = metadata.posts.try_into().map_err(|_| {
            anyhow::anyhow!(
                "{} must contain exactly one <blog-post> metadata entry",
                path.display()
            )
        })?;
        if !valid_id(&meta.id) || meta.title.trim().is_empty() {
            bail!(
                "Article IDs must contain only lowercase letters, digits, and hyphens; titles must not be empty: {}",
                path.display()
            );
        }
        if metadata.states.len() > 1 {
            bail!("Duplicate managed state in {}", path.display());
        }
        let state = metadata
            .states
            .first()
            .map(|value| STANDARD.decode(value).context("Invalid managed state"))
            .transpose()?;
        Ok(RawPost {
            meta,
            path: path.to_owned(),
            body: document.body,
            state,
        })
    }

    pub fn compile_page(&self, path: &Path) -> Result<Page<Compiled>> {
        let document = self.compile_document(path)?;
        let metadata = document.metadata;
        if !metadata.posts.is_empty() || !metadata.states.is_empty() {
            bail!(
                "Standalone pages must not contain article metadata or managed state: {}",
                path.display()
            );
        }
        let [meta]: [PageMeta; 1] = metadata.pages.try_into().map_err(|_| {
            anyhow::anyhow!(
                "{} must contain exactly one <blog-page> metadata entry",
                path.display()
            )
        })?;
        if meta.title.trim().is_empty() {
            bail!("Page titles must not be empty: {}", path.display());
        }
        Ok(Page {
            meta,
            body: document.body,
        })
    }

    fn compile_document(&self, path: &Path) -> Result<CompiledDocument> {
        let canonical = path
            .canonicalize()
            .with_context(|| format!("Reading {}", path.display()))?;
        let root = self.root.canonicalize()?;
        if !canonical.starts_with(root) {
            bail!(
                "Content must remain inside the project root: {}",
                path.display()
            );
        }
        let document_path = format!(
            "/{}",
            path.strip_prefix(self.root)?
                .to_str()
                .context("The document path must be UTF-8")?
        );
        // Defaults live outside the included document, so its own rules can
        // override them without changing relative imports or label ownership.
        let wrapper = self.temporary_file("blog-document-", ".typ")?;
        fs::write(
            wrapper.path(),
            format!(
                "#set raw(theme: bytes({}))\n#include {}\n",
                typst_string(&self.theme),
                typst_string(&document_path)
            ),
        )?;
        let input = wrapper
            .path()
            .to_str()
            .context("The temporary path must be UTF-8")?;
        let metadata: Metadata = serde_json::from_slice(&self.run(&[
            "eval",
            METADATA_QUERY,
            "--in",
            input,
            "--target",
            "html",
        ])?)
        .with_context(|| format!("Invalid metadata in {}", path.display()))?;
        let output = self.temporary_file("blog-html-", ".html")?;
        self.run(&[
            "compile",
            input,
            output
                .path()
                .to_str()
                .context("The temporary path must be UTF-8")?,
        ])?;
        let html = fs::read_to_string(output.path())?;
        Ok(CompiledDocument {
            metadata,
            body: Document::parse(&html),
        })
    }

    fn temporary_file(&self, prefix: &str, suffix: &str) -> Result<tempfile::NamedTempFile> {
        Ok(tempfile::Builder::new()
            .prefix(prefix)
            .suffix(suffix)
            .tempfile_in(self.root.join(".blog"))?)
    }

    fn run(&self, args: &[&str]) -> Result<Vec<u8>> {
        let output = Command::new("typst")
            .args(args)
            .args(["--root", self.root_string, "--features", "html"])
            .args(["--input", &self.canonical_text])
            .current_dir(self.root)
            .output()
            .context("Unable to run Typst; install typst 0.15.1 (brew install typst)")?;
        if !output.status.success() {
            bail!(
                "Typst compilation failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(output.stdout)
    }
}
