//! Browser coordinates and snapshot transport are derived from resolved ranges.
use crate::{config::SiteConfig, content::BuiltPost, fingerprint::digest, storage::Publication};
use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use blog_ranges::RangeReference;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct ArticleManifest<'a> {
    article: &'a str,
    revision: &'a str,
    snapshot: String,
    seed: String,
    prefix: &'a str,
    references: BTreeMap<String, [usize; 2]>,
}

pub(super) fn write(output: &Publication, config: &SiteConfig, post: &BuiltPost) -> Result<String> {
    let snapshot = post.state.state_snapshot()?;
    let snapshot_name = format!(
        "assets/state/{}.{}.bin",
        post.meta.id,
        &digest(&snapshot)[..16]
    );
    output.write(&snapshot_name, &snapshot)?;
    let references = post
        .state
        .range_ids()
        .into_iter()
        .filter_map(|id| {
            let reference = RangeReference {
                schema: 1,
                article: post.meta.id.clone(),
                range: id.clone(),
            };
            post.state.resolve(&reference).ok().map(|selection| {
                let start = post
                    .text
                    .chars()
                    .take(selection.unicode_range.start)
                    .map(char::len_utf16)
                    .sum::<usize>();
                let end = start + selection.text.encode_utf16().count();
                (id, [start, end])
            })
        })
        .collect();
    let manifest = ArticleManifest {
        article: &post.meta.id,
        revision: &post.revision,
        snapshot: format!("{}/{}", config.prefix, snapshot_name),
        seed: STANDARD.encode(&snapshot),
        prefix: &config.prefix,
        references,
    };
    // Match the canonical key ordering used by other JSON artifacts, and make
    // embedding the JSON in a script element safe even for literal HTML text.
    Ok(serde_json::to_string(&serde_json::to_value(manifest)?)?.replace('<', "\\u003c"))
}
