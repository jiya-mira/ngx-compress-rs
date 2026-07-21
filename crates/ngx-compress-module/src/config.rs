#![forbid(unsafe_code)]

//! Safe, Rust-owned location configuration and directive value parsing.

use std::sync::Arc;

use ngx::http::{Merge, MergeConfigError};
use ngx_compress_core::{MimeTypes, StaticMode};

use crate::profile::Profile;

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

impl CompressConfig {
    pub(crate) fn resolve(&self) -> Resolved<'_> {
        let preset = self.profile.and_then(Profile::preset);
        Resolved {
            enabled: self.enable.unwrap_or(false),
            min_length: self
                .min_length
                .or(preset.map(|p| p.min_length))
                .unwrap_or(DEFAULT_MIN_LENGTH),
            vary: self.vary.unwrap_or(true),
            buffer_size: self.buffers.map_or(DEFAULT_BUFFER_SIZE, |(_, size)| size),
            static_mode: self.static_mode.unwrap_or(StaticMode::Off),
            types: self.types.as_deref(),
            gzip: on_level(
                self.gzip.or(preset.map(|p| p.gzip)),
                self.gzip_level.or(preset.map(|p| p.gzip_level)),
                DEFAULT_GZIP_LEVEL,
            ),
            deflate: on_level(
                self.deflate,
                self.deflate_level.or(preset.map(|p| p.deflate_level)),
                DEFAULT_DEFLATE_LEVEL,
            ),
            brotli: self
                .brotli
                .or(preset.map(|p| p.brotli))
                .unwrap_or(false)
                .then(|| {
                    (
                        self.brotli_level
                            .or(preset.map(|p| p.brotli_level))
                            .unwrap_or(DEFAULT_BROTLI_LEVEL),
                        self.brotli_window
                            .or(preset.map(|p| p.brotli_window))
                            .unwrap_or(DEFAULT_BROTLI_WINDOW),
                    )
                }),
            zstd: self
                .zstd
                .or(preset.map(|p| p.zstd))
                .unwrap_or(false)
                .then(|| {
                    self.zstd_level
                        .or(preset.map(|p| p.zstd_level))
                        .unwrap_or(DEFAULT_ZSTD_LEVEL)
                }),
        }
    }

    /// Applies one already-owned TAKE1 directive value.
    pub(crate) fn apply(&mut self, name: &str, value: &str) -> bool {
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

    pub(crate) fn set_buffers(&mut self, count: usize, size: usize) {
        self.buffers = Some((count, size));
    }

    pub(crate) fn set_types(&mut self, values: Vec<String>) {
        self.types = Some(Arc::new(MimeTypes::new(values)));
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

fn on_level(on: Option<bool>, level: Option<u32>, default: u32) -> Option<u32> {
    on.unwrap_or(false).then(|| level.unwrap_or(default))
}

impl Merge for CompressConfig {
    fn merge(&mut self, prev: &Self) -> Result<(), MergeConfigError> {
        merge_opt(&mut self.enable, prev.enable);
        merge_opt(&mut self.profile, prev.profile);
        merge_opt(&mut self.static_mode, prev.static_mode);
        merge_opt(&mut self.gzip, prev.gzip);
        merge_opt(&mut self.gzip_level, prev.gzip_level);
        merge_opt(&mut self.deflate, prev.deflate);
        merge_opt(&mut self.deflate_level, prev.deflate_level);
        merge_opt(&mut self.brotli, prev.brotli);
        merge_opt(&mut self.brotli_level, prev.brotli_level);
        merge_opt(&mut self.brotli_window, prev.brotli_window);
        merge_opt(&mut self.zstd, prev.zstd);
        merge_opt(&mut self.zstd_level, prev.zstd_level);
        merge_opt(&mut self.min_length, prev.min_length);
        merge_opt(&mut self.vary, prev.vary);
        merge_opt(&mut self.buffers, prev.buffers);
        if self.types.is_none() {
            self.types.clone_from(&prev.types);
        }
        Ok(())
    }
}

fn merge_opt<T: Copy>(child: &mut Option<T>, parent: Option<T>) {
    if child.is_none() {
        *child = parent;
    }
}

fn set_static(slot: &mut Option<StaticMode>, value: &str) -> bool {
    let mode = if value.eq_ignore_ascii_case("off") {
        StaticMode::Off
    } else if value.eq_ignore_ascii_case("on") {
        StaticMode::On
    } else if value.eq_ignore_ascii_case("always") {
        StaticMode::Always
    } else {
        return false;
    };
    *slot = Some(mode);
    true
}

fn set_flag(slot: &mut Option<bool>, value: &str) -> bool {
    if value.eq_ignore_ascii_case("on") {
        *slot = Some(true);
        true
    } else if value.eq_ignore_ascii_case("off") {
        *slot = Some(false);
        true
    } else {
        false
    }
}

fn set_u32(slot: &mut Option<u32>, value: &str, min: u32, max: u32) -> bool {
    match value.parse::<u32>() {
        Ok(parsed) if (min..=max).contains(&parsed) => {
            *slot = Some(parsed);
            true
        }
        _ => false,
    }
}

fn set_zstd_level(slot: &mut Option<i32>, value: &str) -> bool {
    match value.parse::<i32>() {
        Ok(parsed) if (-7..=22).contains(&parsed) => {
            *slot = Some(parsed);
            true
        }
        _ => false,
    }
}

fn set_usize(slot: &mut Option<usize>, value: &str) -> bool {
    match value.parse::<usize>() {
        Ok(parsed) => {
            *slot = Some(parsed);
            true
        }
        Err(_) => false,
    }
}
