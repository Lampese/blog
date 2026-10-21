//! A Typst-first static blog with persistent, range-based transclusions.
pub mod build;
pub mod config;
pub mod content;
pub mod dom;
pub mod project;
pub mod server;
pub mod state;

mod fingerprint;
mod locale;
mod site;
mod storage;
