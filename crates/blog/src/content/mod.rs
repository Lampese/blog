//! Article metadata and the Typst source boundary.
mod compiler;
mod receipts;
mod state;

pub(crate) use compiler::TypstCompiler;
pub(crate) use receipts::compact_receipts;
pub(crate) use state::save_state;
pub(crate) use state::with_state;
pub use state::{STATE_END, STATE_START, strip_state};

use crate::dom::{Compiled, Document, ProjectedDocument};
use blog_ranges::{RangeDocument, RangeReference, TextRange, Utf16, typst_string};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostMeta {
    pub id: String,
    pub title: String,
    pub date: NaiveDate,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PageMeta {
    pub title: String,
    #[serde(default)]
    pub summary: String,
}

/// Standalone content has no publication date or article range identity.
pub(crate) struct Page<S> {
    pub meta: PageMeta,
    pub body: Document<S>,
}
/// Typst output before cross-article excerpts are expanded.
pub(crate) struct RawPost {
    pub meta: PostMeta,
    pub path: PathBuf,
    pub body: Document<Compiled>,
    pub state: Option<Vec<u8>>,
}
/// Expanded, annotated article content and the matching range state.
pub struct BuiltPost {
    pub meta: PostMeta,
    pub path: PathBuf,
    pub body: ProjectedDocument,
    pub state: RangeDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    #[serde(flatten)]
    pub range: TextRange<Utf16>,
    pub seed: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Include {
    #[serde(flatten)]
    pub reference: RangeReference,
    #[serde(default)]
    pub receipt: Option<Receipt>,
}
impl Include {
    pub fn code(&self) -> String {
        let r = &self.reference;
        format!(
            "#excerpt((schema: {}, article: {}, range: {}))",
            r.schema,
            typst_string(&r.article),
            typst_string(&r.range)
        )
    }
}
