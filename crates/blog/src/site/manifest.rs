//! Browser coordinates and snapshot transport are derived from resolved ranges.
use crate::{config::SiteConfig, content::BuiltPost, fingerprint::digest, storage::Publication};
use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use blog_ranges::{RangeState, Utf16};
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
    let mut references = BTreeMap::new();
    for registered in post.state.references() {
        let registered = registered?;
        if let RangeState::Valid(selection) = registered.state {
            let range = selection.range.convert::<Utf16>(post.body.text())?;
            references.insert(registered.reference.range, [range.start(), range.end()]);
        }
    }
    let manifest = ArticleManifest {
        article: &post.meta.id,
        revision: post.body.revision(),
        snapshot: format!("{}/{}", config.prefix, snapshot_name),
        seed: STANDARD.encode(&snapshot),
        prefix: &config.prefix,
        references,
    };
    // Match the canonical key ordering used by other JSON artifacts, and make
    // embedding the JSON in a script element safe even for literal HTML text.
    Ok(serde_json::to_string(&serde_json::to_value(manifest)?)?.replace('<', "\\u003c"))
}
