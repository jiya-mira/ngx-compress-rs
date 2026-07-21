//! FFI boundary for `ngx-compress-rs`.
//!
//! This crate isolates the `unsafe` code required to plug Rust filters into the
//! NGINX output chains. Protocol and policy crates stay `unsafe`-free; the
//! primitives here validate their invariants at each boundary and never let a
//! panic unwind across the C ABI.

pub mod filter;
pub mod guard;
