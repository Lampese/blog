//! Initialize a site or create an article without exposing filesystem internals.
use crate::{
    config::{SiteConfig, valid_id},
    storage::atomic_write,
};
use anyhow::{Context, Result, bail};
use blog_ranges::typst_string;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn new_post(root: &Path, slug: &str, title: Option<String>) -> Result<PathBuf> {
    if !valid_id(slug) {
        bail!("The slug must contain only lowercase letters, digits, and hyphens");
    }
    let config = SiteConfig::load(root)?;
    let path = root.join(config.content).join(format!("{slug}.typ"));
    if path.exists() {
        bail!("File already exists: {}", path.display());
    }
    fs::create_dir_all(path.parent().context("The path has no parent directory")?)?;
    let title = title.unwrap_or_else(|| slug.replace('-', " "));
    let date = chrono::Local::now().format("%Y-%m-%d");
    fs::write(
        &path,
        format!(
            "#import \"/typst/blog.typ\": *\n#post(id: {}, title: {}, date: \"{date}\", summary: \"\", tags: (), draft: true)\n\nStart writing here.\n",
            typst_string(slug),
            typst_string(&title)
        ),
    )?;
    eprintln!("Created {} (draft)", path.display());
    Ok(path)
}

pub fn init(directory: &Path) -> Result<()> {
    fs::create_dir_all(directory)?;
    let files = [
        (
            "site.toml",
            include_str!("../../../assets/templates/site.toml"),
        ),
        ("typst/blog.typ", include_str!("../../../typst/blog.typ")),
        (
            "content/posts/about-this-blog.typ",
            include_str!("../../../assets/templates/starter.typ"),
        ),
        (
            "content/about.typ",
            include_str!("../../../assets/templates/about.typ"),
        ),
        (".gitignore", "_site/\n.blog/\n"),
    ];
    for (relative, _) in files {
        let path = directory.join(relative);
        if path.exists() {
            bail!(
                "Initialization will not overwrite an existing file: {}",
                path.display()
            );
        }
    }
    for (relative, value) in files {
        let path = directory.join(relative);
        fs::create_dir_all(path.parent().context("The path has no parent directory")?)?;
        atomic_write(&path, value.as_bytes())?;
    }
    eprintln!(
        "Initialized {}\nRun blog --root {} serve",
        directory.display(),
        directory.display()
    );
    Ok(())
}
