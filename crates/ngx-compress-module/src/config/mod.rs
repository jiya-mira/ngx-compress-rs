#![forbid(unsafe_code)]

//! Safe, Rust-owned location configuration and directive value parsing.

mod merge;
mod profile;
mod resolve;
mod value;

use std::sync::Arc;

use ngx_compress_core::{MimeTypes, StaticMode};

use self::profile::Profile;
use self::value::{set_flag, set_static, set_u32, set_usize, set_zstd_level};

const DEFAULT_GZIP_LEVEL: u32 = 6;
const DEFAULT_DEFLATE_LEVEL: u32 = 6;
const DEFAULT_BROTLI_LEVEL: u32 = 6;
const DEFAULT_BROTLI_WINDOW: u32 = 22;
const DEFAULT_ZSTD_LEVEL: i32 = 3;
const DEFAULT_MIN_LENGTH: usize = 20;
const DEFAULT_BUFFER_SIZE: usize = 8_192;

/// Rust-owned location configuration. `None` means inherit from the parent.
#[derive(Debug, Default)]
pub(crate) struct CompressConfig {
    enable: Option<bool>,
    profile: Option<Profile>,
    static_mode: Option<StaticMode>,
    gzip: Option<bool>,
    gzip_level: Option<u32>,
    deflate: Option<bool>,
    deflate_level: Option<u32>,
    brotli: Option<bool>,
    brotli_level: Option<u32>,
    brotli_window: Option<u32>,
    zstd: Option<bool>,
    zstd_level: Option<i32>,
    min_length: Option<usize>,
    vary: Option<bool>,
    buffers: Option<(usize, usize)>,
    types: Option<Arc<MimeTypes>>,
}

/// Defaults-applied, allocation-free request-path configuration snapshot.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Resolved<'a> {
    pub enabled: bool,
    pub min_length: usize,
    pub vary: bool,
    pub buffer_size: usize,
    pub static_mode: StaticMode,
    pub types: Option<&'a MimeTypes>,
    pub gzip: Option<u32>,
    pub deflate: Option<u32>,
    pub brotli: Option<(u32, u32)>,
    pub zstd: Option<i32>,
}

/// Validated, Rust-owned configuration update produced at the FFI boundary.
pub(crate) enum ConfigUpdate {
    Named { name: String, value: String },
    Buffers { count: usize, size: usize },
    Types(Vec<String>),
}

impl CompressConfig {
    /// Applies one validated, Rust-owned update from the configuration boundary.
    pub(crate) fn apply(&mut self, update: ConfigUpdate) -> bool {
        match update {
            ConfigUpdate::Named { name, value } => self.apply_named(&name, &value),
            ConfigUpdate::Buffers { count, size } => {
                self.buffers = Some((count, size));
                true
            }
            ConfigUpdate::Types(values) => {
                self.types = Some(Arc::new(MimeTypes::new(values)));
                true
            }
        }
    }

    fn apply_named(&mut self, name: &str, value: &str) -> bool {
        match name {
            "compress" => self.set_compress(value),
            "compress_static" => set_static(&mut self.static_mode, value),
            "compress_gzip" => set_flag(&mut self.gzip, value),
            "compress_deflate" => set_flag(&mut self.deflate, value),
            "compress_brotli" => set_flag(&mut self.brotli, value),
            "compress_zstd" => set_flag(&mut self.zstd, value),
            "compress_vary" => set_flag(&mut self.vary, value),
            "compress_gzip_comp_level" => set_u32(&mut self.gzip_level, value, 1, 9),
            "compress_deflate_comp_level" => set_u32(&mut self.deflate_level, value, 1, 9),
            "compress_brotli_comp_level" => set_u32(&mut self.brotli_level, value, 0, 11),
            "compress_brotli_window" => set_u32(&mut self.brotli_window, value, 10, 24),
            "compress_zstd_comp_level" => set_zstd_level(&mut self.zstd_level, value),
            "compress_min_length" => set_usize(&mut self.min_length, value),
            _ => false,
        }
    }

    fn set_compress(&mut self, value: &str) -> bool {
        if value.eq_ignore_ascii_case("off") {
            self.enable = Some(false);
            true
        } else if value.eq_ignore_ascii_case("on") {
            self.enable = Some(true);
            self.profile = Some(Profile::Custom);
            true
        } else if let Some(profile) = Profile::parse(value) {
            self.enable = Some(true);
            self.profile = Some(profile);
            true
        } else {
            false
        }
    }
}
