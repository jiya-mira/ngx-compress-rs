#![forbid(unsafe_code)]

//! Streaming codec adapters for `ngx-compress-rs`.
//!
//! Each compression backend is selected at build time through the crate's
//! per-codec Cargo features, so a build only links the libraries it enables.
//! The `identity` coding is always available because it carries no dependency.

mod identity;

pub use identity::Identity;
