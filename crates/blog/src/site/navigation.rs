//! Article navigation is added after content projection, outside reference text.
use crate::{dom, fingerprint::digest};
use kuchikiki::NodeRef;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Serialize)]
pub(super) struct Section {
    pub id: String,
    pub title: String,
    pub depth: usize,
}

pub(super) fn outline(html: &str) -> (String, Vec<Section>) {
    let root = dom::body(html);
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
    (dom::children_html(&root), sections)
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
