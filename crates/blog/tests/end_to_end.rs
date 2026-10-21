mod fixtures;

use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use blog::{
    build::{self, BuildMode, BuildOptions},
    content, dom,
};
use blog_ranges::TextRange;
use kuchikiki::traits::TendrilSink;
use std::{fs, path::Path};

fn site() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("typst")).unwrap();
    fs::create_dir_all(dir.path().join("content/posts")).unwrap();
    fs::write(
        dir.path().join("typst/blog.typ"),
        include_str!("../../../typst/blog.typ"),
    )
    .unwrap();
    fs::write(
        dir.path().join("site.toml"),
        include_str!("fixtures/site.toml"),
    )
    .unwrap();
    dir
}
fn post(root: &Path, id: &str, body: &str) {
    fs::write(
        root.join(format!("content/posts/{id}.typ")),
        format!(include_str!("fixtures/post.typ"), id = id, body = body),
    )
    .unwrap();
}
fn append(root: &Path, id: &str, body: &str) {
    let path = root.join(format!("content/posts/{id}.typ"));
    let old = fs::read_to_string(&path).unwrap();
    let pos = old.find(content::STATE_START).unwrap_or(old.len());
    fs::write(
        path,
        format!("{}\n{body}\n\n{}", old[..pos].trim_end(), &old[pos..]),
    )
    .unwrap();
}
fn offsets(text: &str, needle: &str) -> (usize, usize) {
    let start = text.find(needle).unwrap();
    let start = text[..start].encode_utf16().count();
    (start, start + needle.encode_utf16().count())
}

fn about(root: &Path) -> Result<std::path::PathBuf> {
    let config = root.join("site.toml");
    fs::write(
        &config,
        format!(
            "{}\nabout = \"profile/about.typ\"\n",
            fs::read_to_string(&config)?
        ),
    )?;
    fs::create_dir_all(root.join("profile"))?;
    fs::create_dir_all(root.join("static/marks"))?;
    fs::write(
        root.join("static/marks/organization.svg"),
        include_str!("fixtures/organization.svg"),
    )?;
    let path = root.join("profile/about.typ");
    fs::write(&path, include_str!("fixtures/about.typ"))?;
    fs::write(
        root.join("profile/intro.typ"),
        fixtures::text("about_intro"),
    )?;
    Ok(path)
}

#[test]
fn standalone_about_page_uses_typst_without_entering_article_chronology() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(root, "a", fixtures::text("body"));
    build::build(root, BuildOptions::default())?;
    assert!(!root.join("_site/about/index.html").exists());
    let home = dom::body(&fs::read_to_string(root.join("_site/index.html"))?);
    assert!(
        home.select_first(".site-header a[href='/blog/about/']")
            .is_err()
    );

    let path = about(root)?;
    let source = fs::read(&path)?;
    let result = build::build(root, BuildOptions::default())?;
    assert_eq!(result.posts.len(), 1);
    assert_eq!(fs::read(&path)?, source);
    let html = fs::read_to_string(root.join("_site/about/index.html"))?;
    let page = kuchikiki::parse_html().one(html);
    assert_eq!(
        page.select_first("h1").unwrap().text_contents(),
        fixtures::text("about_title")
    );
    assert_eq!(
        page.select_first("meta[name=description]")
            .unwrap()
            .attributes
            .borrow()
            .get("content"),
        Some(fixtures::text("about_summary"))
    );
    assert_eq!(
        page.select_first("link[rel=canonical]")
            .unwrap()
            .attributes
            .borrow()
            .get("href"),
        Some("https://example.com/blog/about/")
    );
    assert_eq!(
        page.select_first(".site-header a[aria-current=page]")
            .unwrap()
            .text_contents(),
        fixtures::text("about_navigation")
    );
    assert!(
        page.select_first(".reading-content")
            .unwrap()
            .text_contents()
            .contains(fixtures::text("about_intro"))
    );
    assert!(
        page.select_first(".article-sidebar a[data-section]")
            .is_ok()
    );
    assert!(page.select_first(".reading-content math").is_ok());
    assert!(
        page.select_first("script[src^='/blog/assets/vendor/math.js']")
            .is_ok()
    );
    assert!(
        page.select_first(".reading-content a[href='/blog/articles/']")
            .is_ok()
    );
    assert!(
        page.select_first(
            "iframe.playground[loading=lazy][src^='/blog/assets/playground.html?v=']"
        )
        .is_ok()
    );
    assert!(
        page.select_first(".article-meta, .article-neighbors, #post-data")
            .is_err()
    );
    assert_eq!(page.select(".experience-position").unwrap().count(), 2);
    assert_eq!(
        page.select_first(".experience-dates [data-blog-i18n=present]")
            .unwrap()
            .text_contents(),
        fixtures::text("current_period")
    );
    assert_eq!(
        page.select_first(".experience-position time")
            .unwrap()
            .attributes
            .borrow()
            .get("datetime"),
        Some("2026-09")
    );
    assert!(page.select_first(".experience-marks img[src='/blog/marks/organization.svg'][alt=''][width='32'][height='32']").is_ok());
    assert!(root.join("_site/marks/organization.svg").is_file());
    assert!(
        html_escape::decode_html_entities(&fs::read_to_string(root.join("_site/sitemap.xml"))?)
            .contains("https://example.com/blog/about/")
    );
    assert!(
        !fs::read_to_string(root.join("_site/feed.xml"))?.contains(fixtures::text("about_title"))
    );
    let archive = dom::body(&fs::read_to_string(root.join("_site/articles/index.html"))?);
    assert_eq!(archive.select(".article-list > li").unwrap().count(), 1);

    let config = root.join("site.toml");
    fs::write(
        &config,
        format!("{}\nlanguage = \"en\"\n", fs::read_to_string(&config)?),
    )?;
    build::build(root, BuildOptions::preview())?;
    let page = dom::body(&fs::read_to_string(root.join("_site/about/index.html"))?);
    assert_eq!(
        page.select_first(".site-header a[aria-current=page]")
            .unwrap()
            .text_contents(),
        "About"
    );
    assert_eq!(
        page.select_first(".site-header a[aria-current=page]")
            .unwrap()
            .attributes
            .borrow()
            .get("href"),
        Some("/about/")
    );
    assert!(
        page.select_first(".reading-content a[href='/articles/']")
            .is_ok()
    );
    assert_eq!(
        page.select_first(".experience-dates [data-blog-i18n=present]")
            .unwrap()
            .text_contents(),
        "Present"
    );
    assert_eq!(
        page.select_first("h1").unwrap().text_contents(),
        fixtures::text("about_title")
    );

    fs::write(config, include_str!("fixtures/site.toml"))?;
    build::build(root, BuildOptions::default())?;
    assert!(!root.join("_site/about/index.html").exists());
    assert!(
        !html_escape::decode_html_entities(&fs::read_to_string(root.join("_site/sitemap.xml"))?)
            .contains("/about/")
    );
    Ok(())
}

#[test]
fn about_page_receipts_register_ranges_and_invalid_edits_preserve_output() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(root, "a", fixtures::text("shared_article"));
    let first = build::build(root, BuildOptions::default())?;
    let target = &first.posts["a"];
    let (start, end) = offsets(target.body.text(), fixtures::text("shared_selection"));
    let seed = STANDARD.encode(target.state.state_snapshot()?);
    let path = about(root)?;
    let source = format!(
        "{}\n#excerpt((schema: 1, article: \"a\", range: \"r-about\", receipt: (start: {start}, end: {end}, seed: \"{seed}\")))\n",
        fs::read_to_string(&path)?
    );
    fs::write(&path, &source)?;
    build::build(
        root,
        BuildOptions {
            mode: BuildMode::Check,
            ..Default::default()
        },
    )?;
    assert_eq!(fs::read_to_string(&path)?, source);
    let built = build::build(root, BuildOptions::default())?;
    assert_eq!(built.posts["a"].state.range_ids(), vec!["r-about"]);
    let compacted = fs::read_to_string(&path)?;
    assert!(!compacted.contains("receipt:"));
    assert!(!compacted.contains(content::STATE_START));
    let html = fs::read_to_string(root.join("_site/about/index.html"))?;
    let page = dom::body(&html);
    assert!(
        page.select_first(".transclusion strong")
            .unwrap()
            .text_contents()
            .contains(fixtures::text("shared_selection"))
    );
    assert_eq!(
        page.select_first("a[data-blog-reference]")
            .unwrap()
            .attributes
            .borrow()
            .get("href"),
        Some("/blog/posts/a/?range=r-about")
    );
    build::build(root, BuildOptions::default())?;
    fs::write(&path, format!("{compacted}\n#unknown-about-function()"))?;
    assert!(build::build(root, BuildOptions::default()).is_err());
    assert!(
        build::build(
            root,
            BuildOptions {
                mode: BuildMode::Check,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(
        fs::read_to_string(root.join("_site/about/index.html"))?,
        html
    );
    fs::write(&path, compacted.replace("r-about", "missing-range"))?;
    assert!(build::build(root, BuildOptions::default()).is_err());
    assert_eq!(
        fs::read_to_string(root.join("_site/about/index.html"))?,
        html
    );
    Ok(())
}

fn footer_line(state: &blog_ranges::RangeDocument) -> Result<String> {
    Ok(format!(
        "#metadata({}) <blog-state>",
        blog_ranges::typst_string(&STANDARD.encode(state.state_snapshot()?))
    ))
}

fn conflict_footer(
    body: &str,
    left: &blog_ranges::RangeDocument,
    right: &blog_ranges::RangeDocument,
) -> Result<String> {
    Ok(format!(
        "{}\n\n{}\n<<<<<<< ours\n{}\n=======\n{}\n>>>>>>> theirs\n{}\n",
        body.trim_end(),
        content::STATE_START,
        footer_line(left)?,
        footer_line(right)?,
        content::STATE_END
    ))
}

#[test]
fn compacted_git_footer_conflicts_recover_without_changing_resolved_source_or_output() -> Result<()>
{
    use blog_ranges::RangeDocument;
    let dir = site();
    let root = dir.path();
    post(root, "a", "alpha beta gamma\n\nomega delta");
    let initial = build::build(root, BuildOptions::default())?;
    let initial = &initial.posts["a"];
    let mut basis = RangeDocument::from_snapshot(&initial.state.state_snapshot()?)?;
    let common = basis.register_utf16(TextRange::new(
        offsets(&basis.source(), "beta").0..offsets(&basis.source(), "beta").1,
    ))?;
    let mut left = basis.fork();
    let mut right = basis.fork();
    left.update_source(&format!("left {}", left.source()))?;
    right.update_source(&format!("{} right", right.source()))?;
    let (a, b) = offsets(&left.source(), "gamma");
    let left_reference = left.register_utf16(TextRange::new(a..b))?;
    let (a, b) = offsets(&right.source(), "omega");
    let right_reference = right.register_utf16(TextRange::new(a..b))?;
    post(root, "a", "left alpha beta gamma\n\nomega delta right");
    let path = root.join("content/posts/a.typ");
    let body = fs::read_to_string(&path)?;
    let conflict = conflict_footer(&body, &left, &right)?;
    fs::write(&path, &conflict)?;
    let output = fs::read(root.join("_site/posts/a/index.html"))?;
    assert_eq!(blog::state::inspect(&path)?.snapshots.len(), 2);
    let checked = build::recover_state(root, &path, &[], false)?;
    assert!(!checked.written);
    assert!(checked.backup.is_none());
    assert_eq!(checked.state.ranges.len(), 3);
    assert_eq!(fs::read_to_string(&path)?, conflict);
    assert_eq!(fs::read(root.join("_site/posts/a/index.html"))?, output);
    let applied = build::recover_state(root, &path, &[], true)?;
    assert_eq!(fs::read_to_string(applied.backup.unwrap())?, conflict);
    assert_eq!(
        content::strip_state(&fs::read_to_string(&path)?)?,
        body.trim_end()
    );
    let result = build::build(root, BuildOptions::default())?;
    let state = &result.posts["a"].state;
    assert_eq!(state.resolve(&common)?.text, "beta");
    assert_eq!(state.resolve(&left_reference)?.text, "gamma");
    assert_eq!(state.resolve(&right_reference)?.text, "omega");
    assert_eq!(blog::state::inspect(&path)?.snapshots.len(), 1);
    Ok(())
}

#[test]
fn recovery_preserves_observed_deletion_and_rolls_back_if_a_consumer_is_invalid() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(root, "a", fixtures::text("mixed_language_article"));
    let mut result = build::build(root, BuildOptions::default())?;
    let target = &result.posts["a"];
    let (start, end) = offsets(target.body.text(), fixtures::text("mixed_selection"));
    let revision = target.body.revision().to_owned();
    let reference = build::register(
        root,
        &mut result,
        "a",
        &revision,
        TextRange::new(start..end),
    )?;
    post(root, "b", &reference.code());
    let result = build::build(root, BuildOptions::default())?;
    let left = result.posts["a"].state.fork();
    let mut right = left.fork();
    right.update_source("")?;
    right.update_source(&left.source())?;
    let path = root.join("content/posts/a.typ");
    let body = fs::read_to_string(&path)?;
    let conflict = conflict_footer(content::strip_state(&body)?, &left, &right)?;
    fs::write(&path, &conflict)?;
    let output = fs::read(root.join("_site/posts/b/index.html"))?;
    assert!(build::recover_state(root, &path, &[], true).is_err());
    assert_eq!(fs::read_to_string(&path)?, conflict);
    assert_eq!(fs::read(root.join("_site/posts/b/index.html"))?, output);
    fs::remove_file(root.join("content/posts/b.typ"))?;
    let report = build::recover_state(root, &path, &[], true)?;
    assert!(
        report
            .state
            .ranges
            .iter()
            .all(|range| range.status == "deleted")
    );
    let result = build::build(root, BuildOptions::default())?;
    assert!(matches!(
        result.posts["a"].state.resolve(&reference.reference),
        Err(blog_ranges::ReferenceError::DeletedRange(_))
    ));
    Ok(())
}

#[test]
fn recovery_from_backup_repairs_corrupt_state_but_rejects_unrelated_history() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(root, "a", fixtures::text("stable_article"));
    let result = build::build(root, BuildOptions::default())?;
    let path = root.join("content/posts/a.typ");
    let original = fs::read_to_string(&path)?;
    let backup = root.join(".blog/known-good.typ");
    fs::write(&backup, &original)?;
    let broken = format!(
        "{}\n{}\n#metadata(\"broken\") <blog-state>\n{}\n",
        content::strip_state(&original)?,
        content::STATE_START,
        content::STATE_END
    );
    fs::write(&path, &broken)?;
    let report = build::recover_state(root, &path, std::slice::from_ref(&backup), true)?;
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(
        content::strip_state(&fs::read_to_string(&path)?)?,
        content::strip_state(&original)?
    );
    let other = blog_ranges::RangeDocument::new("a", result.posts["a"].body.text())?;
    fs::write(
        &backup,
        format!(
            "{}\n{}\n{}\n{}\n",
            content::strip_state(&original)?,
            content::STATE_START,
            footer_line(&other)?,
            content::STATE_END
        ),
    )?;
    let before = fs::read(&path)?;
    assert!(build::recover_state(root, &path, &[backup], true).is_err());
    assert_eq!(fs::read(path)?, before);
    Ok(())
}

#[test]
fn sidebar_navigation_keeps_authored_anchors_and_omits_transcluded_headings() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(
        root,
        "a",
        "= Overview <overview>\n\nAlpha. #link(<overview>)[See overview].\n\n== Details\n\nBeta.\n\n= Repeated\n\nGamma.\n\n= Repeated\n\nDelta.",
    );
    let mut result = build::build(root, BuildOptions::default())?;
    let target = &result.posts["a"];
    let (start, end) = offsets(target.body.text(), "Overview");
    let revision = target.body.revision().to_owned();
    let reference = build::register(
        root,
        &mut result,
        "a",
        &revision,
        TextRange::new(start..end),
    )?;
    post(root, "b", &format!("= Local\n\n{}", reference.code()));
    build::build(root, BuildOptions::default())?;
    let parsed =
        kuchikiki::parse_html().one(fs::read_to_string(root.join("_site/posts/a/index.html"))?);
    let links: Vec<_> = parsed
        .select(".article-sidebar a[data-section]")
        .unwrap()
        .collect();
    assert_eq!(links.len(), 4);
    let ids: std::collections::BTreeSet<_> = links
        .iter()
        .map(|link| {
            link.attributes
                .borrow()
                .get("data-section")
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(ids.len(), 4);
    assert!(ids.contains("overview"));
    for id in ids {
        assert_eq!(
            parsed
                .select("#article-content [id]")
                .unwrap()
                .filter(|node| node.attributes.borrow().get("id") == Some(id.as_str()))
                .count(),
            1
        );
    }
    assert!(
        parsed
            .select_first("#article-content .article-sidebar")
            .is_err()
    );
    let consumer =
        kuchikiki::parse_html().one(fs::read_to_string(root.join("_site/posts/b/index.html"))?);
    let links: Vec<_> = consumer
        .select(".article-sidebar a[data-section]")
        .unwrap()
        .collect();
    assert_eq!(links.len(), 1);
    assert!(links[0].text_contents().contains("Local"));
    Ok(())
}

#[test]
fn full_build_reference_edit_delete_and_publish_transaction() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(root, "a", fixtures::text("mixed_language_article"));
    post(root, "b", fixtures::text("second_article"));
    let mut first = build::build(root, BuildOptions::default())?;
    assert!(root.join("_site/feed.xml").exists());
    let home = kuchikiki::parse_html().one(fs::read_to_string(root.join("_site/index.html"))?);
    assert!(
        home.select("a")
            .unwrap()
            .any(|a| a.attributes.borrow().get("href") == Some("/blog/posts/a/"))
    );
    assert!(home.select("script").unwrap().any(|s| {
        s.attributes
            .borrow()
            .get("src")
            .is_some_and(|src| src.starts_with("/blog/assets/site.js?v="))
    }));
    let a = &first.posts["a"];
    let (start, end) = offsets(a.body.text(), fixtures::text("mixed_selection"));
    let revision = a.body.revision().to_owned();
    let reference = build::register(root, &mut first, "a", &revision, TextRange::new(start..end))?;
    append(root, "b", &reference.code());
    let second = build::build(root, BuildOptions::default())?;
    assert!(second.posts["b"].body.html().contains("<strong>"));
    assert!(
        second.posts["b"]
            .body
            .html()
            .contains(fixtures::text("mixed_selection"))
    );
    assert!(second.posts["b"].body.html().contains("transclusion"));
    assert!(fs::read_to_string(root.join("_site/assets/typst.css"))?.contains("math"));
    let path = root.join("content/posts/a.typ");
    let raw = fs::read_to_string(&path)?;
    fs::write(
        &path,
        raw.replace(
            fixtures::text("mixed_selection"),
            fixtures::text("edited_selection"),
        ),
    )?;
    let changed = build::build(root, BuildOptions::default())?;
    assert!(
        changed.posts["b"]
            .body
            .html()
            .contains(fixtures::text("edited_selection"))
    );
    let output = root.join("_site/posts/b/index.html");
    let published = fs::read(&output)?;
    let raw = fs::read_to_string(&path)?;
    fs::write(&path, raw.replace(fixtures::text("edited_selection"), ""))?;
    let error = build::build(root, BuildOptions::default()).err().unwrap();
    assert!(format!("{error:#}").contains("Invalid reference"));
    assert_eq!(fs::read(&output)?, published);
    let raw = fs::read_to_string(&path)?;
    fs::write(
        &path,
        raw.replace("#render[]", fixtures::text("replacement_call")),
    )?;
    assert!(build::build(root, BuildOptions::default()).is_err());
    Ok(())
}

#[test]
fn published_receipt_rebases_and_embeds_formatted_math_without_browser_server() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(root, "a", fixtures::text("math_article"));
    post(root, "b", fixtures::text("excerpt_introduction"));
    let first = build::build(root, BuildOptions::default())?;
    let target = &first.posts["a"];
    let start = target
        .body
        .text()
        .find(fixtures::text("range_start"))
        .unwrap();
    let end = target
        .body
        .text()
        .find(fixtures::text("range_end"))
        .unwrap()
        + fixtures::text("range_end").len();
    let start = target.body.text()[..start].encode_utf16().count();
    let end = target.body.text()[..end].encode_utf16().count();
    let seed = STANDARD.encode(target.state.state_snapshot()?);
    append(
        root,
        "b",
        &format!(
            "#excerpt((schema: 1, article: \"a\", range: \"r-web-test\", receipt: (start: {start}, end: {end}, seed: \"{seed}\")))"
        ),
    );
    let path = root.join("content/posts/a.typ");
    let raw = fs::read_to_string(&path)?;
    fs::write(
        path,
        raw.replace(
            fixtures::text("original_prefix"),
            fixtures::text("extended_prefix"),
        ),
    )?;
    let result = build::build(root, BuildOptions::default())?;
    let html = result.posts["b"].body.html();
    assert!(html.contains("<strong>"));
    assert!(html.contains("<math"));
    assert!(html.contains(fixtures::text("range_start")));
    assert!(html.contains(fixtures::text("range_end")));
    let consumer = fs::read_to_string(root.join("content/posts/b.typ"))?;
    assert!(!content::strip_state(&consumer)?.contains("receipt:"));
    assert!(content::strip_state(&consumer)?.contains("r-web-test"));
    build::build(root, BuildOptions::default())?;
    Ok(())
}

#[test]
fn receipt_compaction_preserves_computed_and_extended_metadata() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(root, "a", fixtures::text("provider_article"));
    let first = build::build(root, BuildOptions::default())?;
    let seed = STANDARD.encode(first.posts["a"].state.state_snapshot()?);
    let receipt = format!("(start: 0, end: 2, seed: \"{seed}\")");
    post(
        root,
        "b",
        &format!(
            r#"
#let ref = (schema: 1, article: "a", range: "computed", receipt: {receipt})
#excerpt(ref)

#excerpt((extra: (article: "a", range: "computed"), schema: 1, article: "a", range: "extended", receipt: {receipt}))
"#
        ),
    );
    let result = build::build(root, BuildOptions::default())?;
    assert_eq!(
        result.posts["a"].state.range_ids(),
        vec!["computed", "extended"]
    );
    let source = fs::read_to_string(root.join("content/posts/b.typ"))?;
    assert!(source.contains("#excerpt(ref)"));
    assert!(source.contains("#excerpt((extra:"));
    assert!(source.contains("range: \"extended\", receipt:"));
    Ok(())
}

#[test]
fn repeated_builds_are_stable_and_check_mode_does_not_write_sources() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(root, "a", fixtures::text("stable_article"));
    build::build(root, BuildOptions::default())?;
    let path = root.join("content/posts/a.typ");
    let source = fs::read(&path)?;
    let page = fs::read(root.join("_site/posts/a/index.html"))?;
    build::build(root, BuildOptions::default())?;
    assert_eq!(fs::read(&path)?, source);
    assert_eq!(fs::read(root.join("_site/posts/a/index.html"))?, page);
    build::build(
        root,
        BuildOptions {
            mode: BuildMode::Check,
            ..Default::default()
        },
    )?;
    assert_eq!(fs::read(&path)?, source);
    Ok(())
}

#[test]
fn cycle_errors_and_drafts_are_enforced() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(
        root,
        "a",
        "#excerpt((schema: 1, article: \"b\", range: \"r-missing\"))",
    );
    post(
        root,
        "b",
        "#excerpt((schema: 1, article: \"a\", range: \"r-missing\"))",
    );
    let error = build::build(root, BuildOptions::default()).err().unwrap();
    assert!(format!("{error:#}").contains("Cyclic transclusion"));
    post(root, "a", fixtures::text("published_article"));
    let path = root.join("content/posts/b.typ");
    post(root, "b", fixtures::text("draft_article"));
    let source = fs::read_to_string(&path)?;
    fs::write(path, source.replace("summary:", "draft: true, summary:"))?;
    assert_eq!(build::build(root, BuildOptions::default())?.posts.len(), 1);
    assert_eq!(
        build::build(
            root,
            BuildOptions {
                drafts: true,
                ..Default::default()
            }
        )?
        .posts
        .len(),
        2
    );
    Ok(())
}

#[test]
fn separate_expansions_do_not_mutate_the_compiled_document_or_each_other() -> Result<()> {
    let compiled = dom::Document::parse(
        "<style>p{color:navy}</style><p>Intro</p><blog-excerpt></blog-excerpt>",
    );
    let original = compiled
        .expand(&[fixtures::text("formatted_html").to_owned()])?
        .project();
    let revision = original.revision().to_owned();
    let replacement = compiled
        .expand(&["<p>Replacement</p>".to_owned()])?
        .project();
    let repeated = compiled
        .expand(&[fixtures::text("formatted_html").to_owned()])?
        .project();
    assert_eq!(original.html(), repeated.html());
    assert_eq!(original.text(), repeated.text());
    assert_eq!(original.revision(), revision);
    assert_eq!(original.styles(), "p{color:navy}");
    assert!(!original.text().contains("Replacement"));
    assert!(replacement.text().contains("Replacement"));
    assert!(!replacement.text().contains(fixtures::text("cjk_word")));
    assert!(compiled.expand(&[]).is_err());
    Ok(())
}

#[test]
fn dom_ranges_keep_formatting_atoms_and_rewrite_links() -> Result<()> {
    let projected = dom::Document::parse(fixtures::text("formatted_html"))
        .expand(&[])?
        .project();
    let start = projected.text().find(fixtures::text("cjk_word")).unwrap();
    let end = projected
        .text()
        .find(fixtures::text("ending_marker"))
        .unwrap();
    let range = projected.text()[..start].chars().count()..projected.text()[..end].chars().count();
    let excerpt = projected.excerpt(TextRange::new(range), "copy", "/posts/a/")?;
    assert!(excerpt.contains("<strong>"));
    assert!(excerpt.contains("<math"));
    assert!(excerpt.contains("href=\"/posts/a/#note\""));
    assert!(!excerpt.contains(fixtures::text("footnote")));
    assert!(
        projected
            .excerpt(TextRange::new(0..usize::MAX), "copy", "/posts/a/")
            .is_err()
    );
    Ok(())
}

#[test]
fn nested_transclusions_follow_a_shared_dependency_regardless_of_file_order() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(root, "z", fixtures::text("shared_article"));
    let mut result = build::build(root, BuildOptions::default())?;
    let revision = result.posts["z"].body.revision().to_owned();
    let (start, end) = offsets(
        result.posts["z"].body.text(),
        fixtures::text("shared_selection"),
    );
    let shared = build::register(
        root,
        &mut result,
        "z",
        &revision,
        TextRange::new(start..end),
    )?;
    for id in ["b", "c"] {
        post(root, id, &shared.code());
    }
    result = build::build(root, BuildOptions::default())?;

    let mut nested = Vec::new();
    for id in ["b", "c"] {
        let revision = result.posts[id].body.revision().to_owned();
        let (start, end) = offsets(
            result.posts[id].body.text(),
            fixtures::text("shared_selection"),
        );
        nested.push(
            build::register(root, &mut result, id, &revision, TextRange::new(start..end))?.code(),
        );
    }
    post(root, "a", &nested.join("\n\n"));
    build::build(root, BuildOptions::default())?;

    let provider = root.join("content/posts/z.typ");
    let source = fs::read_to_string(&provider)?;
    fs::write(
        provider,
        source.replace(
            fixtures::text("shared_selection"),
            fixtures::text("edited_shared_selection"),
        ),
    )?;
    let updated = build::build(root, BuildOptions::default())?;
    assert_eq!(
        updated.posts["a"]
            .body
            .text()
            .matches(fixtures::text("edited_shared_selection"))
            .count(),
        2
    );
    for id in ["b", "c"] {
        assert!(
            updated.posts[id]
                .body
                .text()
                .contains(fixtures::text("edited_shared_selection"))
        );
    }
    Ok(())
}

#[test]
fn changing_ui_language_preserves_reference_state_and_browser_coordinates() -> Result<()> {
    fn browser_text(node: &kuchikiki::NodeRef, output: &mut String) {
        if let Some(value) = node.as_text() {
            output.push_str(&value.borrow());
            return;
        }
        if let Some(element) = node.as_element() {
            let attributes = element.attributes.borrow();
            if let Some(start) = attributes.get("data-blog-start") {
                assert_eq!(
                    start.parse::<usize>().unwrap(),
                    output.encode_utf16().count()
                );
            }
            if let Some(token) = attributes.get("data-blog-token") {
                output.push_str(token);
                return;
            }
        }
        for child in node.children() {
            browser_text(&child, output);
        }
    }

    let dir = site();
    let root = dir.path();
    post(
        root,
        "a",
        "#theorem(title: \"Identity\")[A body with $x=x$.] <identity>\n\n#proof[This follows from identity.]\n\nSee @identity.\n\n#playground(\"console.log(42);\")",
    );
    let mut result = build::build(root, BuildOptions::default())?;
    let revision = result.posts["a"].body.revision().to_owned();
    let length = result.posts["a"].body.text().encode_utf16().count();
    let reference = build::register(root, &mut result, "a", &revision, TextRange::new(0..length))?;
    post(root, "b", &reference.code());
    let chinese = build::build(root, BuildOptions::default())?;
    let sources: Vec<_> = ["a", "b"]
        .map(|id| fs::read(root.join(format!("content/posts/{id}.typ"))).unwrap())
        .into();
    let config = root.join("site.toml");
    fs::write(
        &config,
        format!("{}\nlanguage = \"en-US\"\n", fs::read_to_string(&config)?),
    )?;
    let english = build::build(root, BuildOptions::default())?;
    for (index, id) in ["a", "b"].into_iter().enumerate() {
        assert_eq!(
            chinese.posts[id].body.revision(),
            english.posts[id].body.revision()
        );
        assert_eq!(chinese.posts[id].body.text(), english.posts[id].body.text());
        assert_eq!(
            chinese.posts[id].state.state_snapshot()?,
            english.posts[id].state.state_snapshot()?
        );
        assert_eq!(
            sources[index],
            fs::read(root.join(format!("content/posts/{id}.typ")))?
        );
        let page = kuchikiki::parse_html().one(fs::read_to_string(
            root.join(format!("_site/posts/{id}/index.html")),
        )?);
        assert_eq!(
            page.select_first("html")
                .unwrap()
                .attributes
                .borrow()
                .get("lang"),
            Some("en-US")
        );
        let mut projected = String::new();
        browser_text(
            page.select_first("#article-content").unwrap().as_node(),
            &mut projected,
        );
        assert_eq!(projected, english.posts[id].body.text());
        let title = page
            .select_first(".statement-title")
            .unwrap()
            .text_contents();
        assert!(title.contains("Theorem 1 (Identity)"), "{title}");
        let frame = page.select_first("iframe.playground").unwrap();
        assert_eq!(
            frame.attributes.borrow().get("title"),
            Some("Runnable JavaScript example")
        );
    }
    let page =
        kuchikiki::parse_html().one(fs::read_to_string(root.join("_site/posts/b/index.html"))?);
    assert_eq!(
        page.select_first("[data-blog-i18n=excerpt_source]")
            .unwrap()
            .text_contents(),
        "From"
    );
    let sandbox = kuchikiki::parse_html().one(fs::read_to_string(
        root.join("_site/assets/playground.html"),
    )?);
    assert_eq!(sandbox.select_first("#run").unwrap().text_contents(), "Run");
    assert_eq!(
        sandbox
            .select_first("html")
            .unwrap()
            .attributes
            .borrow()
            .get("lang"),
        Some("en-US")
    );
    fs::create_dir_all(root.join("assets/i18n"))?;
    fs::write(root.join("assets/i18n/en-US.json"), r#"{"run":"__STOP__"}"#)?;
    build::build(root, BuildOptions::default())?;
    let sandbox = kuchikiki::parse_html().one(fs::read_to_string(
        root.join("_site/assets/playground.html"),
    )?);
    assert_eq!(
        sandbox.select_first("#run").unwrap().text_contents(),
        "__STOP__"
    );
    let messages: serde_json::Value =
        serde_json::from_str(&sandbox.select_first("#site-text").unwrap().text_contents())?;
    assert_eq!(messages["messages"]["run"], "__STOP__");
    Ok(())
}

#[test]
fn code_highlighting_preserves_text_references_and_article_overrides() -> Result<()> {
    let dir = site();
    let root = dir.path();
    fs::write(
        root.join("content/posts/nearby.txt"),
        fixtures::text("relative_file"),
    )?;
    let rust = fixtures::text("rust_code");
    post(
        root,
        "a",
        &format!(include_str!("fixtures/highlighting.typ"), rust = rust),
    );
    post(
        root,
        "plain",
        "#set raw(theme: none)\n```rust\nfn main() {}\n```",
    );
    let mut first = build::build(root, BuildOptions::default())?;
    let a = &first.posts["a"];
    let parsed = kuchikiki::parse_html().one(a.body.html().to_owned());
    let code = parsed.select_first("code[data-lang=rust]").unwrap();
    assert_eq!(code.text_contents(), rust);
    for language in ["rust", "haskell", "javascript", "python", "typst", "sh"] {
        assert!(
            parsed
                .select(&format!("code[data-lang={language}] span[style]"))
                .unwrap()
                .count()
                > 0,
            "{language}"
        );
    }
    assert_eq!(
        parsed
            .select_first("code[data-lang=unknownlanguage]")
            .unwrap()
            .text_contents(),
        fixtures::text("unknown_language_code")
    );
    assert!(a.body.text().contains(fixtures::text("relative_file")));
    let plain = kuchikiki::parse_html().one(first.posts["plain"].body.html().to_owned());
    assert_eq!(plain.select("code span[style]").unwrap().count(), 0);
    let original_text = a.body.text().to_owned();
    let original_revision = a.body.revision().to_owned();
    let (start, end) = offsets(a.body.text(), fixtures::text("rust_selection"));
    let reference = build::register(
        root,
        &mut first,
        "a",
        &original_revision,
        TextRange::new(start..end),
    )?;
    post(root, "b", &reference.code());
    build::build(root, BuildOptions::default())?;
    fs::create_dir(root.join("assets"))?;
    fs::write(
        root.join("assets/code.tmTheme"),
        include_str!("../../../assets/code.tmTheme").replace("#285f7a", "#123456"),
    )?;
    let changed = build::build(root, BuildOptions::default())?;
    assert_eq!(changed.posts["a"].body.text(), original_text);
    assert_eq!(changed.posts["a"].body.revision(), original_revision);
    assert!(changed.posts["a"].body.html().contains("#123456"));
    assert_eq!(
        changed.posts["a"].state.resolve(&reference.reference)?.text,
        fixtures::text("rust_selection")
    );
    let excerpt = kuchikiki::parse_html().one(changed.posts["b"].body.html().to_owned());
    assert_eq!(
        excerpt.select_first("pre code").unwrap().text_contents(),
        fixtures::text("rust_selection")
    );
    assert!(excerpt.select("pre code span[style]").unwrap().count() > 0);
    Ok(())
}

#[test]
fn configuration_cannot_replace_source_or_repository_directories() {
    let dir = site();
    let root = dir.path();
    post(root, "a", fixtures::text("body"));
    for output in [
        ".",
        "../outside",
        "content",
        "crates",
        "static",
        ".git/output",
    ] {
        fs::write(root.join("site.toml"), format!("output = \"{output}\"\n")).unwrap();
        assert!(
            build::build(root, BuildOptions::default()).is_err(),
            "{output}"
        );
    }
    fs::write(root.join("site.toml"), "output = \"handwritten\"\n").unwrap();
    fs::create_dir(root.join("handwritten")).unwrap();
    fs::write(
        root.join("handwritten/keep.txt"),
        fixtures::text("preserved_file"),
    )
    .unwrap();
    assert!(build::build(root, BuildOptions::default()).is_err());
    assert_eq!(
        fs::read_to_string(root.join("handwritten/keep.txt")).unwrap(),
        fixtures::text("preserved_file")
    );
}

#[test]
fn about_configuration_keeps_source_separate_from_articles_and_output() -> Result<()> {
    let dir = site();
    let root = dir.path();
    for path in [
        "content/posts/about.typ",
        "../about.typ",
        "profile/about.html",
    ] {
        fs::write(root.join("site.toml"), format!("about = \"{path}\"\n"))?;
        assert!(blog::config::SiteConfig::load(root).is_err(), "{path}");
    }
    fs::write(
        root.join("site.toml"),
        "about = \"profile/about.typ\"\noutput = \"profile\"\n",
    )?;
    assert!(blog::config::SiteConfig::load(root).is_err());
    Ok(())
}

#[test]
fn rich_typst_content_and_preview_have_stable_projection() -> Result<()> {
    let dir = site();
    let root = dir.path();
    fs::write(
        root.join("diagram.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40"><rect width="100" height="40" fill="darkred"/></svg>"#,
    )?;
    fs::write(
        root.join("references.yml"),
        "knuth:\n  type: Book\n  title: The Art of Computer Programming\n  author: Knuth, Donald E.\n  date: 1968\n",
    )?;
    post(root, "a", include_str!("fixtures/rich-content.typ"));
    let production = build::build(root, BuildOptions::default())?;
    let html = fs::read_to_string(root.join("_site/posts/a/index.html"))?;
    assert!(html.contains("data:image/svg+xml") || html.contains("<svg"));
    assert!(html.contains("/blog/demos/example.html"));
    assert!(html.contains("data-code="));
    assert!(html.contains("/blog/assets/playground.html"));
    assert!(root.join("_site/assets/vendor/playground.js").exists());
    assert!(root.join("_site/assets/vendor/math.js").exists());
    assert!(root.join("_site/assets/vendor/math-latex.js").exists());
    let fonts = fs::read_to_string(root.join("_site/assets/fonts/zhuque/subsets.css"))?;
    assert!(fonts.contains("unicode-range:"));
    assert!(root.join("_site/assets/fonts/zhuque").read_dir()?.count() > 1);
    assert!(
        kuchikiki::parse_html()
            .one(html.clone())
            .select("script")
            .unwrap()
            .any(|script| {
                script
                    .attributes
                    .borrow()
                    .get("src")
                    .is_some_and(|src| src.starts_with("/blog/assets/vendor/math.js?v="))
            })
    );
    assert!(html.contains("equation-number"));
    assert!(html.contains("Computer Programming"));
    let preview = build::build(
        root,
        BuildOptions {
            mode: BuildMode::Preview,
            ..Default::default()
        },
    )?;
    assert_eq!(
        production.posts["a"].body.text(),
        preview.posts["a"].body.text()
    );
    assert_eq!(
        production.posts["a"].body.revision(),
        preview.posts["a"].body.revision()
    );
    assert!(
        fs::read_to_string(root.join("_site/posts/a/index.html"))?
            .contains("src=\"/demos/example.html\"")
    );
    Ok(())
}

#[test]
fn playground_source_and_sandbox_survive_transclusion_without_inline_execution() -> Result<()> {
    let dir = site();
    let root = dir.path();
    let code = fixtures::text("playground_code");
    post(
        root,
        "a",
        &format!("#playground({})", blog_ranges::typst_string(code)),
    );
    let mut result = build::build(root, BuildOptions::default())?;
    let a = &result.posts["a"];
    let atom_end = a.body.text().find('\u{fffd}').unwrap() + '\u{fffd}'.len_utf8();
    let (start, end) = offsets(a.body.text(), &a.body.text()[..atom_end]);
    let revision = a.body.revision().to_owned();
    let reference = build::register(
        root,
        &mut result,
        "a",
        &revision,
        TextRange::new(start..end),
    )?;
    post(root, "b", &reference.code());
    build::build(root, BuildOptions::default())?;
    for id in ["a", "b"] {
        let html = fs::read_to_string(root.join(format!("_site/posts/{id}/index.html")))?;
        let parsed = kuchikiki::parse_html().one(html);
        let frame = parsed.select_first("iframe.playground").unwrap();
        let attrs = frame.attributes.borrow();
        assert_eq!(attrs.get("data-code"), Some(code));
        assert!(
            attrs
                .get("src")
                .is_some_and(|src| src.starts_with("/blog/assets/playground.html?v="))
        );
        assert_eq!(attrs.get("sandbox"), Some("allow-scripts"));
        assert!(!attrs.contains("srcdoc"));
    }
    let html = fs::read_to_string(root.join("_site/assets/playground.html"))?;
    assert!(!html.contains("__PLAYGROUND_VERSION__"));
    assert!(!html.contains("__MONO_FONT_DATA__"));
    assert!(html.contains("data:font/woff2;base64,"));
    assert!(root.join("_site/assets/vendor/licenses.txt").exists());
    let consumer =
        kuchikiki::parse_html().one(fs::read_to_string(root.join("_site/posts/b/index.html"))?);
    assert!(
        consumer
            .select_first("a[data-blog-reference][aria-haspopup=dialog]")
            .is_ok()
    );
    assert!(root.join("_site/assets/references.js").exists());
    Ok(())
}

#[test]
fn changing_article_identity_with_retained_state_is_rejected() -> Result<()> {
    let dir = site();
    let root = dir.path();
    post(root, "a", fixtures::text("body"));
    build::build(root, BuildOptions::default())?;
    let path = root.join("content/posts/a.typ");
    let source = fs::read_to_string(&path)?;
    fs::write(path, source.replace("id: \"a\"", "id: \"renamed\""))?;
    let error = build::build(root, BuildOptions::default()).err().unwrap();
    assert!(format!("{error:#}").contains("Article ID does not match reference state"));
    Ok(())
}

#[test]
fn cli_initialization_and_new_article_work_without_overwriting() -> Result<()> {
    use std::process::Command;
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("new-site");
    let binary = env!("CARGO_BIN_EXE_blog");
    assert!(
        Command::new(binary)
            .arg("init")
            .arg(&root)
            .status()?
            .success()
    );
    assert!(
        Command::new(binary)
            .arg("--root")
            .arg(&root)
            .args(["new", "a-note", "--title", fixtures::text("article_title")])
            .status()?
            .success()
    );
    assert!(
        !Command::new(binary)
            .arg("--root")
            .arg(&root)
            .args(["new", "a-note"])
            .status()?
            .success()
    );
    let built = build::build(
        &root,
        BuildOptions {
            drafts: true,
            ..Default::default()
        },
    )?;
    assert_eq!(built.posts.len(), 2);
    assert_eq!(
        built.posts["a-note"].meta.title,
        fixtures::text("article_title")
    );
    assert!(built.posts["a-note"].meta.draft);
    assert!(root.join("content/about.typ").is_file());
    assert!(root.join("_site/about/index.html").is_file());
    assert!(!fs::read_to_string(root.join("site.toml"))?.contains("Lampese"));
    assert!(!fs::read_to_string(root.join("content/about.typ"))?.contains("Lampese"));
    Ok(())
}

#[test]
#[cfg(unix)]
fn preview_exits_on_ctrl_c_with_an_open_live_reload_connection() -> Result<()> {
    use std::{
        io::Write,
        net::{TcpListener, TcpStream},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let dir = site();
    post(dir.path(), "a", fixtures::text("preview_article"));
    let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
    let mut child = Command::new(env!("CARGO_BIN_EXE_blog"))
        .arg("--root")
        .arg(dir.path())
        .args(["serve", "--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let stream = loop {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            break Some(stream);
        }
        if child.try_wait()?.is_some() || Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    let Some(mut stream) = stream else {
        let _ = child.kill();
        let _ = child.wait();
        anyhow::bail!("Preview failed to start");
    };
    stream.write_all(
        format!("GET /_blog/events HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes(),
    )?;
    // Wait for SSE response headers so the persistent connection is established.
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut response = [0u8; 1024];
    use std::io::Read;
    let received = stream.read(&mut response)?;
    assert!(String::from_utf8_lossy(&response[..received]).contains("200 OK"));
    assert!(
        String::from_utf8_lossy(&response[..received])
            .to_ascii_lowercase()
            .contains("cache-control: no-store")
    );
    assert!(
        Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()?
            .success()
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait()? {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("Preview did not exit after Ctrl-C");
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    Ok(())
}
