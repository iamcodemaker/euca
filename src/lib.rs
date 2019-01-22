#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/euca/0.1.0")]

//! Modular wasm application framework.

pub mod patch;
pub mod diff;
pub mod vdom;
pub mod app;

pub use diff::diff;
