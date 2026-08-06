//! Defaults-applied request-path configuration snapshots.

use ngx_compress_core::StaticMode;

use crate::{CompressConfig, Profile, ResolveConfig, Resolved};

use super::{
    DEFAULT_BROTLI_LEVEL, DEFAULT_BROTLI_WINDOW, DEFAULT_BUFFER_SIZE, DEFAULT_DEFLATE_LEVEL,
    DEFAULT_GZIP_LEVEL, DEFAULT_MIN_LENGTH, DEFAULT_ZSTD_LEVEL,
};

/// A preset bundle applied when a named profile is selected.
#[derive(Clone, Copy, Debug)]
struct Preset {
    gzip: bool,
    brotli: bool,
    zstd: bool,
    gzip_level: u32,
    deflate_level: u32,
    brotli_level: u32,
    brotli_window: u32,
    zstd_level: i32,
    min_length: usize,
}

impl Profile {
    /// Returns the tuning bundle for a named tier; `Custom` contributes none.
    fn preset(self) -> Option<Preset> {
        let preset = match self {
            Self::Custom => return None,
            Self::Fast => Preset {
                gzip: true,
                brotli: true,
                zstd: true,
                gzip_level: 4,
                deflate_level: 4,
                brotli_level: 4,
                brotli_window: 18,
                zstd_level: 3,
                min_length: 256,
            },
            Self::Balanced => Preset {
                gzip: true,
                brotli: true,
                zstd: true,
                gzip_level: 6,
                deflate_level: 6,
                brotli_level: 5,
                brotli_window: 22,
                zstd_level: 6,
                min_length: 256,
            },
            Self::Max => Preset {
                gzip: true,
                brotli: true,
                zstd: true,
                gzip_level: 9,
                deflate_level: 9,
                brotli_level: 11,
                brotli_window: 24,
                zstd_level: 19,
                min_length: 128,
            },
        };
        Some(preset)
    }
}

impl ResolveConfig for CompressConfig {
    fn resolve(&self) -> Resolved<'_> {
        let preset = self.profile.and_then(Profile::preset);
        Resolved {
            enabled: self.enable.unwrap_or(false),
            min_length: self
                .min_length
                .or(preset.map(|p| p.min_length))
                .unwrap_or(DEFAULT_MIN_LENGTH),
            vary: self.vary.unwrap_or(true),
            stats_mode: self.stats_mode.unwrap_or_default(),
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
}

fn on_level(on: Option<bool>, level: Option<u32>, default: u32) -> Option<u32> {
    on.unwrap_or(false).then(|| level.unwrap_or(default))
}
