//! Unicode content samples are data, separate from the test implementation.
use std::{collections::BTreeMap, sync::OnceLock};

pub fn text(key: &str) -> &'static str {
    static SAMPLES: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    SAMPLES
        .get_or_init(|| {
            serde_json::from_str(include_str!("unicode.json")).expect("Valid Unicode fixtures")
        })
        .get(key)
        .expect("Known Unicode fixture")
}
