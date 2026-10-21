//! Immutable canonical content ties HTML, text, styles, and revision together.
use super::{
    Document, Expanded, publication,
    tree::{body, children_html, deep_clone, element, is_atom, skipped, tag},
};
use crate::{fingerprint::digest, locale::Locale};
use anyhow::Result;
use blog_ranges::{Scalar, TextRange, Utf16};
use kuchikiki::NodeRef;

#[derive(Debug)]
pub struct ProjectedDocument {
    html: String,
    text: String,
    revision: String,
    styles: String,
}

impl ProjectedDocument {
    pub fn html(&self) -> &str {
        &self.html
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }
    pub fn styles(&self) -> &str {
        &self.styles
    }

    pub(crate) fn prepare(
        &self,
        locale: &Locale,
        playground: &str,
        prefix: &str,
    ) -> Result<publication::PreparedDocument> {
        publication::prepare(body(&self.html), locale, playground, prefix)
    }

    pub fn excerpt(
        &self,
        range: TextRange<Scalar>,
        prefix: &str,
        source_url: &str,
    ) -> Result<String> {
        let range = range.convert::<Utf16>(&self.text)?;
        let root = body(&self.html);
        let out = NodeRef::new_document();
        for child in root.children() {
            if let Some(copy) = selected(&child, &range)? {
                out.append(copy);
            }
        }
        let mut ids = std::collections::HashSet::new();
        for node in out.descendants() {
            if let Some(el) = node.as_element()
                && let Some(id) = el.attributes.borrow().get("id")
            {
                ids.insert(id.to_owned());
            }
        }
        for node in out.descendants() {
            if let Some(el) = node.as_element() {
                let mut attrs = el.attributes.borrow_mut();
                if let Some(id) = attrs.get("id").map(str::to_owned) {
                    attrs.insert("id", format!("{prefix}-{id}"));
                }
                if let Some(target) = attrs
                    .get("href")
                    .and_then(|s| s.strip_prefix('#'))
                    .map(str::to_owned)
                {
                    attrs.insert(
                        "href",
                        if ids.contains(&target) {
                            format!("#{prefix}-{target}")
                        } else {
                            format!("{source_url}#{target}")
                        },
                    );
                }
            }
        }
        Ok(children_html(&out))
    }
}

impl Document<Expanded> {
    pub fn project(self) -> ProjectedDocument {
        for node in self.root.descendants() {
            if let Some(el) = node.as_element() {
                let mut attrs = el.attributes.borrow_mut();
                for key in ["data-blog-start", "data-blog-text", "data-blog-token"] {
                    attrs.remove(key);
                }
            }
        }
        let mut projection = ProjectionBuffer::default();
        annotate(&self.root, &mut projection);
        ProjectedDocument {
            html: children_html(&self.root),
            revision: digest(&projection.text),
            text: projection.text,
            styles: self.styles,
        }
    }
}

#[derive(Default)]
struct ProjectionBuffer {
    text: String,
    utf16: usize,
}

impl ProjectionBuffer {
    fn push(&mut self, value: &str) {
        self.utf16 += value.encode_utf16().count();
        self.text.push_str(value);
    }
}

fn annotate(node: &NodeRef, text: &mut ProjectionBuffer) {
    if skipped(node) {
        return;
    }
    if is_atom(node) {
        let canonical = deep_clone(node);
        canonical
            .as_element()
            .unwrap()
            .attributes
            .borrow_mut()
            .remove("data-blog-i18n-title");
        let token = format!("\u{fffc}{}\u{fffd}", &digest(canonical.to_string())[..24]);
        let el = node.as_element().unwrap();
        let mut attrs = el.attributes.borrow_mut();
        attrs.insert("data-blog-start", text.utf16.to_string());
        attrs.insert("data-blog-token", token.clone());
        text.push(&token);
        return;
    }
    for child in node.children().collect::<Vec<_>>() {
        if let Some(value) = child.as_text() {
            let value = value.borrow().clone();
            if value.is_empty() {
                continue;
            }
            let span = element("span");
            {
                let mut a = span.as_element().unwrap().attributes.borrow_mut();
                a.insert("data-blog-text", "".into());
                a.insert("data-blog-start", text.utf16.to_string());
            }
            child.insert_before(span.clone());
            span.append(child.clone());
            text.push(&value);
        } else {
            annotate(&child, text);
        }
    }
    if matches!(
        tag(node),
        "p" | "h1" | "h2" | "h3" | "h4" | "li" | "pre" | "blockquote" | "table" | "figure" | "div"
    ) {
        let span = element("span");
        {
            let mut a = span.as_element().unwrap().attributes.borrow_mut();
            a.insert("data-blog-text", "".into());
            a.insert("data-blog-start", text.utf16.to_string());
            a.insert("class", "text-boundary".into());
        }
        span.append(NodeRef::new_text("\n"));
        node.append(span);
        text.push("\n");
    }
}
fn selected(node: &NodeRef, range: &TextRange<Utf16>) -> Result<Option<NodeRef>> {
    if skipped(node) {
        return Ok(None);
    }
    if let Some(el) = node.as_element() {
        let attrs = el.attributes.borrow();
        if let Some(start) = attrs.get("data-blog-start") {
            let start: usize = start.parse()?;
            let atom = attrs.get("data-blog-token");
            let value = atom
                .map(str::to_owned)
                .unwrap_or_else(|| node.text_contents());
            let end = start + value.encode_utf16().count();
            if end <= range.start() || start >= range.end() {
                return Ok(None);
            }
            if atom.is_some() {
                return Ok(Some(deep_clone(node)));
            }
            if attrs.contains("data-blog-text") {
                let a = range.start().saturating_sub(start);
                let b = (range.end() - start).min(end - start);
                let value = TextRange::<Utf16>::new(a..b).slice(&value)?.to_owned();
                let copy = NodeRef::new(node.data().clone());
                copy.append(NodeRef::new_text(value));
                return Ok(Some(copy));
            }
        }
    }
    let copy = NodeRef::new(node.data().clone());
    for child in node.children() {
        if let Some(child) = selected(&child, range)? {
            copy.append(child);
        }
    }
    if copy.first_child().is_some() {
        Ok(Some(copy))
    } else {
        Ok(None)
    }
}
