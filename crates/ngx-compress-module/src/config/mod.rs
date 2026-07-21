#![forbid(unsafe_code)]

//! Safe, Rust-owned location configuration and directive value parsing.

mod merge;
mod resolve;
mod value;

const DEFAULT_GZIP_LEVEL: u32 = 6;
const DEFAULT_DEFLATE_LEVEL: u32 = 6;
const DEFAULT_BROTLI_LEVEL: u32 = 6;
const DEFAULT_BROTLI_WINDOW: u32 = 22;
const DEFAULT_ZSTD_LEVEL: i32 = 3;
const DEFAULT_MIN_LENGTH: usize = 20;
const DEFAULT_BUFFER_SIZE: usize = 8_192;
