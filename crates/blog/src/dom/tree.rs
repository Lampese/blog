//! Internal DOM ownership and structural operations.
use kuchikiki::{NodeRef, traits::*};

pub fn body(html: &str) -> NodeRef {
    kuchikiki::parse_html()
        .one(html)
        .select_first("body")
        .unwrap()
        .as_node()
        .clone()
}
pub(super) fn children_html(node: &NodeRef) -> String {
    node.children().map(|n| n.to_string()).collect()
}
pub(super) fn element(tag: &str) -> NodeRef {
    kuchikiki::parse_html()
        .one(format!("<{tag}></{tag}>"))
        .select_first(tag)
        .unwrap()
        .as_node()
        .clone()
}
pub(super) fn tag(node: &NodeRef) -> &str {
    node.as_element()
        .map(|e| e.name.local.as_ref())
        .unwrap_or("")
}
pub(super) fn is_atom(node: &NodeRef) -> bool {
    matches!(
        tag(node),
        "math" | "svg" | "img" | "video" | "audio" | "iframe" | "canvas"
    )
}
pub(super) fn skipped(node: &NodeRef) -> bool {
    matches!(tag(node), "script" | "style" | "template")
}
pub(super) fn deep_clone(node: &NodeRef) -> NodeRef {
    let copy = NodeRef::new(node.data().clone());
    for child in node.children() {
        copy.append(deep_clone(&child));
    }
    copy
}
