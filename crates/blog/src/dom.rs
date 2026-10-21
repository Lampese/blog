use crate::{fingerprint::digest, locale::Locale};
use anyhow::{Result, bail};
use kuchikiki::{NodeRef, traits::*};
use std::ops::Range;

pub struct Projection {
    pub html: String,
    pub text: String,
}
pub fn body(html: &str) -> NodeRef {
    kuchikiki::parse_html()
        .one(html)
        .select_first("body")
        .unwrap()
        .as_node()
        .clone()
}
pub fn children_html(node: &NodeRef) -> String {
    node.children().map(|n| n.to_string()).collect()
}
pub fn head_styles(html: &str) -> String {
    let parsed = kuchikiki::parse_html().one(html);
    parsed
        .select("head style")
        .unwrap()
        .map(|n| n.text_contents())
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn prefix_urls(html: &str, prefix: &str) -> String {
    let root = body(html);
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
    children_html(&root)
}
pub fn version_playgrounds(html: &str, version: &str) -> String {
    let root = body(html);
    for node in root.select("iframe.playground").unwrap() {
        let mut attrs = node.attributes.borrow_mut();
        if !attrs.contains("loading") {
            attrs.insert("loading", "lazy".to_owned());
        }
        if attrs.get("src") == Some("/assets/playground.html") {
            attrs.insert("src", format!("/assets/playground.html?v={version}"));
        }
    }
    children_html(&root)
}

/// Translate generated labels after projection, retaining their canonical text
/// for browser selections. Labels are selected as complete generated objects.
pub(crate) fn localize(html: &str, locale: &Locale) -> Result<String> {
    let root = body(html);
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
    Ok(children_html(&root))
}
pub fn element(tag: &str) -> NodeRef {
    kuchikiki::parse_html()
        .one(format!("<{tag}></{tag}>"))
        .select_first(tag)
        .unwrap()
        .as_node()
        .clone()
}
fn tag(node: &NodeRef) -> &str {
    node.as_element()
        .map(|e| e.name.local.as_ref())
        .unwrap_or("")
}
fn is_atom(node: &NodeRef) -> bool {
    matches!(
        tag(node),
        "math" | "svg" | "img" | "video" | "audio" | "iframe" | "canvas"
    )
}
fn skipped(node: &NodeRef) -> bool {
    matches!(tag(node), "script" | "style" | "template")
}
pub fn project(html: &str) -> Projection {
    let root = body(html);
    // Remove old annotations from embedded excerpts before assigning article-local offsets.
    for node in root.descendants().collect::<Vec<_>>() {
        if let Some(el) = node.as_element() {
            let mut attrs = el.attributes.borrow_mut();
            for key in ["data-blog-start", "data-blog-text", "data-blog-token"] {
                attrs.remove(key);
            }
        }
    }
    let mut text = String::new();
    annotate(&root, &mut text);
    Projection {
        html: children_html(&root),
        text,
    }
}
fn annotate(node: &NodeRef, text: &mut String) {
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
        attrs.insert("data-blog-start", text.encode_utf16().count().to_string());
        attrs.insert("data-blog-token", token.clone());
        text.push_str(&token);
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
                a.insert("data-blog-start", text.encode_utf16().count().to_string());
            }
            child.insert_before(span.clone());
            span.append(child.clone());
            text.push_str(&value);
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
            a.insert("data-blog-start", text.encode_utf16().count().to_string());
            a.insert("class", "text-boundary".into());
        }
        span.append(NodeRef::new_text("\n"));
        node.append(span);
        text.push('\n');
    }
}
pub fn excerpt(
    html: &str,
    projection: &str,
    range: Range<usize>,
    prefix: &str,
    source_url: &str,
) -> Result<String> {
    let start = projection
        .chars()
        .take(range.start)
        .collect::<String>()
        .encode_utf16()
        .count();
    let end = projection
        .chars()
        .take(range.end)
        .collect::<String>()
        .encode_utf16()
        .count();
    let root = body(html);
    let out = NodeRef::new_document();
    for child in root.children() {
        if let Some(copy) = selected(&child, start..end)? {
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
fn selected(node: &NodeRef, range: Range<usize>) -> Result<Option<NodeRef>> {
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
            if end <= range.start || start >= range.end {
                return Ok(None);
            }
            if atom.is_some() {
                return Ok(Some(deep_clone(node)));
            }
            if attrs.contains("data-blog-text") {
                let a = range.start.saturating_sub(start);
                let b = (range.end - start).min(end - start);
                let units: Vec<u16> = value.encode_utf16().collect();
                let value = String::from_utf16(&units[a..b])?;
                let copy = NodeRef::new(node.data().clone());
                copy.append(NodeRef::new_text(value));
                return Ok(Some(copy));
            }
        }
    }
    let copy = NodeRef::new(node.data().clone());
    for child in node.children() {
        if let Some(child) = selected(&child, range.clone())? {
            copy.append(child);
        }
    }
    if copy.first_child().is_some() {
        Ok(Some(copy))
    } else {
        Ok(None)
    }
}
fn deep_clone(node: &NodeRef) -> NodeRef {
    let copy = NodeRef::new(node.data().clone());
    for child in node.children() {
        copy.append(deep_clone(&child));
    }
    copy
}
pub fn reference_nodes(html: &str) -> Result<Vec<serde_json::Value>> {
    let root = body(html);
    let mut refs = Vec::new();
    for node in root.descendants() {
        if tag(&node) == "blog-excerpt" {
            let attrs = node.as_element().unwrap().attributes.borrow();
            let value = attrs
                .get("data-reference")
                .ok_or_else(|| anyhow::anyhow!("Excerpt is missing reference metadata"))?;
            refs.push(serde_json::from_str(value)?);
        }
    }
    Ok(refs)
}
pub fn replace_excerpts(html: &str, replacements: &[String]) -> Result<String> {
    let root = body(html);
    let nodes: Vec<_> = root
        .descendants()
        .filter(|n| tag(n) == "blog-excerpt")
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
    Ok(children_html(&root))
}
