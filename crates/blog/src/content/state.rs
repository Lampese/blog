//! The managed footer contains the latest CRDT state, not a separate database.
use crate::storage::atomic_write;
use anyhow::{Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use blog_ranges::{RangeDocument, typst_string};
use std::{fs, path::Path};

pub const STATE_START: &str = "// @blog-state:begin";
pub const STATE_END: &str = "// @blog-state:end";

pub fn strip_state(raw: &str) -> Result<&str> {
    if let Some(start) = raw.find(STATE_START) {
        let after = &raw[start + STATE_START.len()..];
        if !after.contains(STATE_END) || !raw[start..].trim_end().ends_with(STATE_END) {
            bail!("Invalid managed state footer; restore the @blog-state region from Git");
        }
        Ok(raw[..start].trim_end())
    } else {
        Ok(raw.trim_end())
    }
}
pub(crate) fn save_state(path: &Path, state: &RangeDocument) -> Result<()> {
    let raw = fs::read_to_string(path)?;
    let next = with_state(strip_state(&raw)?, state)?;
    if raw != next {
        atomic_write(path, next.as_bytes())?;
    }
    Ok(())
}

pub(crate) fn with_state(body: &str, state: &RangeDocument) -> Result<String> {
    let encoded = STANDARD.encode(state.state_snapshot()?);
    Ok(format!(
        "{}\n\n{STATE_START}\n#metadata({}) <blog-state>\n{STATE_END}\n",
        body.trim_end(),
        typst_string(&encoded)
    ))
}
