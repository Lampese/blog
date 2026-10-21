//! Content digests version artifacts; metadata signatures detect source changes.
use crate::config::SiteConfig;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::{path::Path, time::UNIX_EPOCH};

pub(crate) fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

pub(crate) fn source_tree(root: &Path, config: &SiteConfig) -> Result<String> {
    let mut values = Vec::new();
    for path in [
        root.join(&config.content),
        root.join("typst"),
        root.join("static"),
        root.join("assets"),
        root.join("site.toml"),
    ]
    .into_iter()
    .chain(config.about.as_ref().map(|path| root.join(path)))
    {
        if !path.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(path).sort_by_file_name() {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let meta = entry.metadata()?;
            let stamp = meta.modified()?.duration_since(UNIX_EPOCH)?.as_nanos();
            values.push(format!("{}:{}:{stamp}", entry.path().display(), meta.len()));
        }
    }
    Ok(digest(values.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_page_changes_and_removal_invalidate_the_preview() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let config = SiteConfig {
            about: Some("profile/about.typ".into()),
            ..Default::default()
        };
        std::fs::create_dir(root.join("profile"))?;
        let path = root.join("profile/about.typ");
        let absent = source_tree(root, &config)?;
        std::fs::write(&path, "An introduction.")?;
        let first = source_tree(root, &config)?;
        assert_ne!(absent, first);
        std::fs::write(&path, "An updated introduction.")?;
        let updated = source_tree(root, &config)?;
        assert_ne!(first, updated);
        std::fs::remove_file(path)?;
        assert_ne!(updated, source_tree(root, &config)?);
        Ok(())
    }
}
