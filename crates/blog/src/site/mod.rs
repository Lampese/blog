//! Publish a resolved article graph as pages, assets, and machine-readable indexes.
mod assets;
mod indexes;
mod manifest;
mod pages;

use crate::{
    config::SiteConfig,
    content::{BuiltPost, Page, PostMeta},
    dom::Expanded,
    fingerprint::digest,
    locale::Locale,
    storage::Publication,
};
use anyhow::Result;
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};

#[derive(Serialize)]
struct PostView<'a> {
    #[serde(flatten)]
    meta: &'a PostMeta,
    url: String,
    canonical: String,
    minutes: usize,
    tag_links: Vec<(String, String)>,
}
pub(super) fn post_url(config: &SiteConfig, id: &str) -> String {
    format!("{}/posts/{id}/", config.prefix)
}
fn tag_url(config: &SiteConfig, tag: &str) -> String {
    format!("{}/tags/{}/", config.prefix, &digest(tag)[..12])
}
fn view<'a>(config: &SiteConfig, post: &'a BuiltPost) -> PostView<'a> {
    PostView {
        meta: &post.meta,
        url: post_url(config, &post.meta.id),
        canonical: format!(
            "{}/posts/{}/",
            config.base_url.trim_end_matches('/'),
            post.meta.id
        ),
        minutes: post.body.text().chars().count().div_ceil(450).max(1),
        tag_links: post
            .meta
            .tags
            .iter()
            .map(|tag| (tag.clone(), tag_url(config, tag)))
            .collect(),
    }
}

struct Catalog<'a> {
    config: &'a SiteConfig,
    about: Option<&'a Page<Expanded>>,
    ordered: Vec<&'a BuiltPost>,
    text: &'a Locale,
}

impl<'a> Catalog<'a> {
    fn new(
        config: &'a SiteConfig,
        posts: &'a BTreeMap<String, BuiltPost>,
        about: Option<&'a Page<Expanded>>,
        text: &'a Locale,
    ) -> Self {
        let mut ordered: Vec<_> = posts.values().collect();
        ordered.sort_by(|a, b| {
            b.meta
                .date
                .cmp(&a.meta.date)
                .then(a.meta.id.cmp(&b.meta.id))
        });
        Self {
            config,
            about,
            ordered,
            text,
        }
    }
}

pub(crate) fn publish(
    root: &Path,
    config: &SiteConfig,
    posts: &BTreeMap<String, BuiltPost>,
    about: Option<&Page<Expanded>>,
    text: &Locale,
    preview: bool,
) -> Result<()> {
    let output = Publication::new(root, &config.output)?;
    let catalog = Catalog::new(config, posts, about, text);
    let styles = posts
        .values()
        .map(|post| post.body.styles())
        .chain(about.map(|page| page.body.styles()));
    let assets = assets::Assets::load(root, styles, text)?;
    pages::write(root, &output, &catalog, &assets, preview)?;
    assets.write(&output)?;
    indexes::write(&output, &catalog)?;
    output.copy_static(&root.join("static"))?;
    output.commit()
}
