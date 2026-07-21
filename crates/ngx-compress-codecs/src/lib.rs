#![forbid(unsafe_code)]

//! Streaming codec adapters for `ngx-compress-rs`.
//!
//! Each compression backend is selected at build time through the crate's
//! per-codec Cargo features, so a build only links the libraries it enables.
//! The `identity` coding is always available because it carries no dependency.
//! The `vendored` (self-compiled, SIMD) and `system-libs` (distro shared
//! libraries) backends are mutually exclusive.

#[cfg(all(feature = "vendored", feature = "system-libs"))]
compile_error!("enable exactly one codec backend: `vendored` or `system-libs`");

mod identity;
pub use identity::Identity;

#[cfg(any(feature = "gzip", feature = "deflate"))]
mod flate;
#[cfg(feature = "deflate")]
pub use flate::Deflate;
#[cfg(feature = "gzip")]
pub use flate::Gzip;

#[cfg(feature = "brotli")]
mod brotli_codec;
#[cfg(feature = "brotli")]
pub use brotli_codec::Brotli;

#[cfg(feature = "zstd")]
mod zstd_codec;
#[cfg(feature = "zstd")]
pub use zstd_codec::Zstd;
