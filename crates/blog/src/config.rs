use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteConfig {
    pub title: String,
    pub description: String,
    pub author: String,
    pub base_url: String,
    pub language: String,
    pub content: String,
    pub about: Option<String>,
    pub output: String,
    #[serde(skip_deserializing)]
    pub prefix: String,
}
impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            title: "Notes".into(),
            description: "Notes on technology, mathematics, and everyday ideas.".into(),
            author: "Author".into(),
            base_url: "https://example.com".into(),
            language: "zh-CN".into(),
            content: "content/posts".into(),
            about: None,
            output: "_site".into(),
            prefix: String::new(),
        }
    }
}
impl SiteConfig {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("site.toml");
        let mut config: Self = toml::from_str(
            &fs::read_to_string(&path).with_context(|| format!("Reading {}", path.display()))?,
        )?;
        safe_relative(&config.content)?;
        safe_relative(&config.output)?;
        if let Some(about) = &config.about {
            let path = safe_relative(about)?;
            if path.starts_with(&config.content) || path.extension().is_none_or(|ext| ext != "typ")
            {
                bail!("The about page must be a .typ file outside the article directory");
            }
        }
        let output = Path::new(&config.output);
        for protected in [
            config.content.as_str(),
            "crates",
            "typst",
            "assets",
            "static",
            ".git",
            ".github",
            "target",
            ".blog",
        ]
        .into_iter()
        .chain(config.about.as_deref())
        {
            let path = Path::new(protected);
            if path.starts_with(output) || output.starts_with(path) {
                bail!(
                    "The output directory must not overlap project files: {}",
                    config.output
                );
            }
        }
        let uri: axum::http::Uri = config
            .base_url
            .parse()
            .context("base_url is not a valid URL")?;
        if !matches!(uri.scheme_str(), Some("https" | "http"))
            || uri.authority().is_none()
            || uri.query().is_some()
            || config.base_url.contains('#')
        {
            bail!("base_url must be an http(s) URL with a host and no query or fragment");
        }
        config.prefix = uri.path().trim_end_matches('/').to_owned();
        Ok(config)
    }
}
pub fn safe_relative(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if value.is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
        bail!("The path must be relative to the project root: {value}");
    }
    Ok(path.to_owned())
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}
