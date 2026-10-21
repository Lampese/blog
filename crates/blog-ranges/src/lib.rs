//! Persistent, overlapping text references backed by Loro's native marks.
mod coordinates;
pub mod references;
pub use coordinates::{Byte, Scalar, TextRange, Unit, Utf16};
pub use references::{
    RangeDocument, RangeReference, RangeState, ReferenceError, RegisteredRange, Selection,
};

#[derive(Debug, thiserror::Error)]
pub enum AnchorError {
    #[error("range must be nonempty and within the document")]
    InvalidRange,
    #[error("UTF-16 offset is outside the text or splits a surrogate pair")]
    InvalidUtf16Offset,
}

pub fn typst_string(value: &str) -> String {
    let mut result = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            ch if ch.is_control() => result.push_str(&format!("\\u{{{:x}}}", ch as u32)),
            ch => result.push(ch),
        }
    }
    result.push('"');
    result
}
