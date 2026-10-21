//! Presentation changes preserve the canonical projection's coordinates.
use super::{Document, Expanded, tree::children_html};
use crate::{fingerprint::digest, locale::Locale};
use anyhow::Result;
use kuchikiki::NodeRef;
use serde::Serialize;
use std::collections::BTreeSet;

pub(crate) struct PreparedDocument {
    pub html: String,
    pub sections: Vec<Section>,
}

pub(super) fn prepare(
    root: NodeRef,
    locale: &Locale,
    playground: &str,
    prefix: &str,
) -> Result<PreparedDocument> {
    version_playgrounds(&root, playground);
    localize(&root, locale)?;
    prefix_urls(&root, prefix);
    let sections = outline(&root);
    Ok(PreparedDocument {
        html: children_html(&root),
        sections,
    })
}

impl Document<Expanded> {
    pub(crate) fn prepare(
        &self,
        locale: &Locale,
        playground: &str,
        prefix: &str,
    ) -> Result<PreparedDocument> {
        prepare(
            super::tree::deep_clone(&self.root),
            locale,
            playground,
            prefix,
        )
    }
}

fn prefix_urls(root: &NodeRef, prefix: &str) {
    if !prefix.is_empty() {
        for node in root.descendants() {
            if let Some(el) = node.as_element() {
                let mut attrs = el.attributes.borrow_mut();
                for key in ["href", "src", "poster"] {
                    if let Some(value) = attrs.get(key).map(str::to_owned)
                        && value.starts_with('/')
                        && !value.starts_with("//")
                        && !value.starts_with(&format!("{prefix}/"))
                    {
                        attrs.insert(key, format!("{prefix}{value}"));
                    }
                }
            }
        }
    }
}
fn version_playgrounds(root: &NodeRef, version: &str) {
    for node in root.select("iframe.playground").unwrap() {
        let mut attrs = node.attributes.borrow_mut();
        if !attrs.contains("loading") {
            attrs.insert("loading", "lazy".to_owned());
        }
        if attrs.get("src") == Some("/assets/playground.html") {
            attrs.insert("src", format!("/assets/playground.html?v={version}"));
        }
    }
}

/// Translate generated labels after projection, retaining their canonical text
/// for browser selections. Labels are selected as complete generated objects.
fn localize(root: &NodeRef, locale: &Locale) -> Result<()> {
    let labels: Vec<_> = root.select("[data-blog-i18n]").unwrap().collect();
    for label in labels {
        let key = label
            .attributes
            .borrow()
            .get("data-blog-i18n")
            .unwrap()
            .to_owned();
        let canonical = label.text_contents();
        let start = label
            .as_node()
            .select_first("[data-blog-start]")
            .ok()
            .and_then(|node| {
                node.attributes
                    .borrow()
                    .get("data-blog-start")
                    .map(str::to_owned)
            });
        for child in label.as_node().children().collect::<Vec<_>>() {
            child.detach();
        }
        label
            .as_node()
            .append(NodeRef::new_text(locale.text(&key)?));
        if let Some(start) = start {
            let mut attributes = label.attributes.borrow_mut();
            attributes.insert("data-blog-start", start);
            attributes.insert("data-blog-token", canonical);
        }
    }
    for node in root.select("[data-blog-i18n-title]").unwrap() {
        let key = node
            .attributes
            .borrow()
            .get("data-blog-i18n-title")
            .unwrap()
            .to_owned();
        node.attributes
            .borrow_mut()
            .insert("title", locale.text(&key)?);
    }
    Ok(())
}

#[derive(Serialize)]
pub(crate) struct Section {
    pub id: String,
    pub title: String,
    pub depth: usize,
}

fn outline(root: &NodeRef) -> Vec<Section> {
    let mut used: BTreeSet<_> = root
        .select("[id]")
        .unwrap()
        .map(|node| node.attributes.borrow().get("id").unwrap().to_owned())
        .collect();
    let mut sections = Vec::new();
    for heading in root.select("h1,h2,h3,h4,h5,h6").unwrap() {
        if heading.as_node().ancestors().any(excerpt) {
            continue;
        }
        let title = heading.text_contents().trim().to_owned();
        if title.is_empty() {
            continue;
        }
        let mut attrs = heading.attributes.borrow_mut();
        let id = if let Some(id) = attrs.get("id").filter(|id| !id.is_empty()) {
            id.to_owned()
        } else {
            let base = format!("section-{}", &digest(&title)[..12]);
            let mut id = base.clone();
            let mut suffix = 2;
            while !used.insert(id.clone()) {
                id = format!("{base}-{suffix}");
                suffix += 1;
            }
            attrs.insert("id", id.clone());
            id
        };
        let level = heading.name.local.as_ref()[1..].parse::<usize>().unwrap();
        sections.push(Section {
            id,
            title,
            depth: level,
        });
    }
    let minimum = sections
        .iter()
        .map(|section| section.depth)
        .min()
        .unwrap_or(1);
    for section in &mut sections {
        section.depth -= minimum;
    }
    sections
}

fn excerpt(node: NodeRef) -> bool {
    node.as_element().is_some_and(|element| {
        element
            .attributes
            .borrow()
            .get("class")
            .is_some_and(|classes| {
                classes
                    .split_whitespace()
                    .any(|class| class == "transclusion")
            })
    })
}
