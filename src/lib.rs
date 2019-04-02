#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/euca/0.1.0")]

//! Modular wasm application framework.

pub mod patch;
pub mod diff;
pub mod dom;
pub mod vdom;
pub mod app;
pub mod route;
pub mod component;

#[cfg(feature = "typed-html")]
pub mod typed_html;

pub use diff::diff;
pub use app::AppBuilder;
pub use component::ComponentBuilder;

pub use app::model;

#[doc(hidden)]
pub mod test;
