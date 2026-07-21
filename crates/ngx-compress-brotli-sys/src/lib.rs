//! `br` (Brotli) coding backed by the system `libbrotlienc` shared library.
//!
//! Only the C encoder API is declared and called here — nothing is compiled or
//! exported. The symbols resolve at final link against the distro's shared
//! `libbrotlienc`/`libbrotlicommon`, which the NGINX build adds via
//! `-lbrotlienc -lbrotlicommon`. The step logic mirrors the vendored pure-Rust
//! adapter; only the provider of the C symbols differs.

mod encoder;

pub use encoder::SystemBrotli;
