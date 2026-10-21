//! Search, RSS, sitemap, and crawler files share one chronological catalog.
use super::{Catalog, post_url};
use crate::storage::Publication;
use anyhow::Result;
use chrono::{DateTime, Utc};

pub(super) fn write(output: &Publication, catalog: &Catalog<'_>) -> Result<()> {
    let config = catalog.config;
    let ordered = &catalog.ordered;
    let search: Vec<_> = ordered.iter().map(|p| serde_json::json!({"title":p.meta.title,"summary":p.meta.summary,"tags":p.meta.tags,"text":p.body.text(),"url":post_url(config,&p.meta.id)})).collect();
    output.write("search.json", serde_json::to_string(&search)?)?;
    let esc = |s: &str| html_escape::encode_safe(s).into_owned();
    let mut feed = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\"><channel><title>{}</title><link>{}</link><description>{}</description>",
        esc(&config.title),
        esc(&config.base_url),
        esc(&config.description)
    );
    for post in ordered {
        let link = format!(
            "{}/posts/{}/",
            config.base_url.trim_end_matches('/'),
            post.meta.id
        );
        let date = DateTime::<Utc>::from_naive_utc_and_offset(
            post.meta.date.and_hms_opt(0, 0, 0).unwrap(),
            Utc,
        )
        .to_rfc2822();
        feed.push_str(&format!("<item><title>{}</title><link>{}</link><guid>{}</guid><pubDate>{}</pubDate><description>{}</description></item>",esc(&post.meta.title),esc(&link),esc(&link),date,esc(&post.meta.summary)));
    }
    feed.push_str("</channel></rss>");
    output.write("feed.xml", feed)?;
    let mut sitemap = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">",
    );
    for path in std::iter::once(String::new())
        .chain(std::iter::once("articles/".into()))
        .chain(catalog.about.map(|_| "about/".into()))
        .chain(ordered.iter().map(|p| format!("posts/{}/", p.meta.id)))
    {
        sitemap.push_str(&format!(
            "<url><loc>{}/{}</loc></url>",
            esc(config.base_url.trim_end_matches('/')),
            path
        ));
    }
    sitemap.push_str("</urlset>");
    output.write("sitemap.xml", sitemap)?;
    output.write(
        "robots.txt",
        format!(
            "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
            config.base_url.trim_end_matches('/')
        ),
    )?;
    output.write(".nojekyll", "")?;
    Ok(())
}
