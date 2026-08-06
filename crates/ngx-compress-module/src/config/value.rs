//! Typed parsing for already-owned NGINX directive values.

use std::sync::Arc;

use ngx_compress_core::{MimeTypes, StaticMode};

use crate::{ApplyConfig, CompressConfig, ConfigUpdate, Profile, StatsMode};

impl ApplyConfig for CompressConfig {
    /// Applies one validated, Rust-owned update from the configuration boundary.
    fn apply(&mut self, update: ConfigUpdate) -> bool {
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
}

impl CompressConfig {
    fn apply_named(&mut self, name: &str, value: &str) -> bool {
        match name {
            "compress" => self.set_compress(value),
            "compress_static" => set_static(&mut self.static_mode, value),
            "compress_gzip" => set_flag(&mut self.gzip, value),
            "compress_deflate" => set_flag(&mut self.deflate, value),
            "compress_brotli" => set_flag(&mut self.brotli, value),
            "compress_zstd" => set_flag(&mut self.zstd, value),
            "compress_vary" => set_flag(&mut self.vary, value),
            "compress_stats" => set_stats_mode(&mut self.stats_mode, value),
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

fn set_stats_mode(slot: &mut Option<StatsMode>, value: &str) -> bool {
    let mode = if value.eq_ignore_ascii_case("off") {
        StatsMode::Off
    } else if value.eq_ignore_ascii_case("variables") {
        StatsMode::Variables
    } else if value.eq_ignore_ascii_case("server_timing") {
        StatsMode::ServerTiming
    } else {
        return false;
    };
    *slot = Some(mode);
    true
}

impl Profile {
    fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("fast") {
            Some(Self::Fast)
        } else if value.eq_ignore_ascii_case("balanced") {
            Some(Self::Balanced)
        } else if value.eq_ignore_ascii_case("max") {
            Some(Self::Max)
        } else {
            None
        }
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
