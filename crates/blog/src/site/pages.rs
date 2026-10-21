//! HTML templates consume view models; they do not load or mutate article state.
use super::{Catalog, assets::Assets, manifest, navigation, view};
use crate::{
    dom,
    fingerprint::digest,
    storage::{Publication, read_text_override},
};
use anyhow::Result;
use minijinja::{Environment, context, value::Kwargs};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(super) fn write(
    root: &Path,
    output: &Publication,
    catalog: &Catalog<'_>,
    assets: &Assets,
    preview: bool,
) -> Result<()> {
    let config = catalog.config;
    let mut env = Environment::new();
    env.add_global("development", preview);
    env.add_global("has_about", catalog.about.is_some());
    env.add_global("locale_json", catalog.text.script_data()?);
    let locale = catalog.text.clone();
    env.add_function(
        "t",
        move |key: String, kwargs: Kwargs| -> Result<String, minijinja::Error> {
            let arguments = kwargs
                .args()
                .map(|name| {
                    Ok((
                        name.to_owned(),
                        kwargs.get::<minijinja::Value>(name)?.to_string(),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, minijinja::Error>>()?;
            locale.format(&key, &arguments).map_err(|error| {
                minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, error.to_string())
            })
        },
    );
    env.add_global(
        "asset_versions",
        minijinja::Value::from_serialize(&assets.versions),
    );
    for (name, fallback) in [
        (
            "base.html",
            include_str!("../../../../assets/templates/base.html"),
        ),
        (
            "home.html",
            include_str!("../../../../assets/templates/home.html"),
        ),
        (
            "list.html",
            include_str!("../../../../assets/templates/list.html"),
        ),
        (
            "post.html",
            include_str!("../../../../assets/templates/post.html"),
        ),
        (
            "page.html",
            include_str!("../../../../assets/templates/page.html"),
        ),
        (
            "contents.html",
            include_str!("../../../../assets/templates/contents.html"),
        ),
        (
            "404.html",
            include_str!("../../../../assets/templates/404.html"),
        ),
    ] {
        let source = read_text_override(root, &format!("assets/templates/{name}"), fallback)?;
        env.add_template_owned(name.to_owned(), source)?;
    }
    let ordered = &catalog.ordered;
    let views: Vec<_> = ordered.iter().map(|p| view(config, p)).collect();
    let mut site = config.clone();
    for project in &mut site.projects {
        if project.url.starts_with('/') && !project.url.starts_with("//") {
            project.url = format!("{}{}", site.prefix, project.url);
        }
    }
    let render = |template: &str, context: minijinja::Value| -> Result<String> {
        Ok(env.get_template(template)?.render(context)?)
    };
    let prepare_content = |html: &str| -> Result<_> {
        let content = dom::prefix_urls(
            &dom::localize(
                &dom::version_playgrounds(html, &assets.versions.playground),
                catalog.text,
            )?,
            &config.prefix,
        );
        Ok(navigation::outline(&content))
    };
    output.write(
        "index.html",
        render(
            "home.html",
            context!(
                site => site,
                posts => views,
                title => config.title,
                description => config.description,
                canonical => format!("{}/", config.base_url.trim_end_matches('/')),
            ),
        )?,
    )?;
    output.write(
        "articles/index.html",
        render(
            "list.html",
            context!(
                site => site,
                posts => views,
                title => catalog.text.text("articles")?,
                description => config.description,
                canonical => format!("{}/articles/", config.base_url.trim_end_matches('/')),
            ),
        )?,
    )?;
    let tags: BTreeSet<_> = ordered.iter().flat_map(|p| p.meta.tags.iter()).collect();
    for tag in &tags {
        let tagged: Vec<_> = ordered
            .iter()
            .filter(|p| p.meta.tags.contains(tag))
            .map(|p| view(config, p))
            .collect();
        output.write(
            format!("tags/{}/index.html", &digest(tag)[..12]),
            render(
                "list.html",
                context!(
                    site => site,
                    posts => tagged,
                    title => tag,
                    description => catalog.text.format("tag_description", &BTreeMap::from([("tag".to_owned(), tag.to_string())]))?,
                    canonical => format!("{}/tags/{}/", config.base_url.trim_end_matches('/'), &digest(tag)[..12]),
                ),
            )?,
        )?;
    }
    for (index, post) in ordered.iter().enumerate() {
        let manifest = manifest::write(output, config, post)?;
        let newer = index.checked_sub(1).map(|i| view(config, ordered[i]));
        let older = ordered.get(index + 1).map(|p| view(config, p));
        let (content, sections) = prepare_content(&post.html)?;
        output.write(
            format!("posts/{}/index.html", post.meta.id),
            render(
                "post.html",
                context!(
                    site => site,
                    post => view(config, post),
                    title => post.meta.title,
                    description => post.meta.summary,
                    canonical => format!("{}/posts/{}/", config.base_url.trim_end_matches('/'), post.meta.id),
                    content => content,
                    sections => sections,
                    manifest => manifest,
                    newer => newer,
                    older => older,
                ),
            )?,
        )?;
    }
    if let Some(page) = catalog.about {
        let (content, sections) = prepare_content(&page.html)?;
        output.write(
            "about/index.html",
            render(
                "page.html",
                context!(
                    site => site,
                    is_about => true,
                    standalone_page => true,
                    title => page.meta.title,
                    description => if page.meta.summary.is_empty() { &config.description } else { &page.meta.summary },
                    summary => page.meta.summary,
                    canonical => format!("{}/about/", config.base_url.trim_end_matches('/')),
                    content => content,
                    sections => sections,
                ),
            )?,
        )?;
    }
    output.write(
        "404.html",
        render(
            "404.html",
            context!(site => site, title => catalog.text.text("not_found")?, description => catalog.text.text("not_found")?),
        )?,
    )?;
    Ok(())
}
