//! Parsed documents expand before projection; projected content is immutable.
mod projection;
mod publication;
mod tree;

pub use projection::ProjectedDocument;
pub use tree::body;

use crate::content::Include;
use anyhow::{Context, Result, bail};
use kuchikiki::{NodeRef, traits::*};
use std::marker::PhantomData;
use tree::{deep_clone, tag};

pub struct Compiled;
pub struct Expanded;

/// A parsed body whose phase controls which content operations are available.
/// ```compile_fail
/// use blog::dom::{Compiled, Document};
/// let compiled = Document::<Compiled>::parse("<p>body</p>");
/// let projection = compiled.project();
/// ```
pub struct Document<S> {
    root: NodeRef,
    styles: String,
    phase: PhantomData<S>,
}

impl<S> Document<S> {
    pub(crate) fn styles(&self) -> &str {
        &self.styles
    }
}

impl Document<Compiled> {
    pub fn parse(html: &str) -> Self {
        let document = kuchikiki::parse_html().one(html);
        let styles = document
            .select("head style")
            .unwrap()
            .map(|node| node.text_contents())
            .collect::<Vec<_>>()
            .join("\n");
        let root = document.select_first("body").unwrap().as_node().clone();
        Self {
            root,
            styles,
            phase: PhantomData,
        }
    }

    pub(crate) fn includes(&self) -> Result<Vec<Include>> {
        self.root
            .descendants()
            .filter(|node| tag(node) == "blog-excerpt")
            .map(|node| {
                let attrs = node.as_element().unwrap().attributes.borrow();
                let value = attrs
                    .get("data-reference")
                    .context("Excerpt is missing reference metadata")?;
                serde_json::from_str(value).context("Invalid reference metadata")
            })
            .collect()
    }

    pub fn expand(&self, replacements: &[String]) -> Result<Document<Expanded>> {
        let root = deep_clone(&self.root);
        let nodes: Vec<_> = root
            .descendants()
            .filter(|node| tag(node) == "blog-excerpt")
            .collect();
        if nodes.len() != replacements.len() {
            bail!("Excerpt count does not match the replacement count");
        }
        for (node, replacement) in nodes.into_iter().zip(replacements) {
            for child in body(replacement).children().collect::<Vec<_>>() {
                node.insert_before(child);
            }
            node.detach();
        }
        Ok(Document {
            root,
            styles: self.styles.clone(),
            phase: PhantomData,
        })
    }
}
