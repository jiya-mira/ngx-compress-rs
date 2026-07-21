//! Location configuration and the `compress_*` directive setters.
//!
//! Every field is an `Option` so `None` means "inherit"; a child location can
//! both enable and disable a setting. One data-driven setter dispatches on the
//! directive name to keep the command table flat.

use core::ffi::{c_char, c_void};
use core::mem::size_of;
use core::ptr::NonNull;

use ngx::core::{NGX_CONF_ERROR, NGX_CONF_OK};
use ngx::ffi::{
    NGX_LOG_EMERG, ngx_array_create, ngx_array_push, ngx_array_t, ngx_command_t, ngx_conf_t,
    ngx_parse_size, ngx_str_t,
};
use ngx::http::{Merge, MergeConfigError};
use ngx::ngx_conf_log_error;

/// Default per-codec compression levels (aligned with the upstream modules).
const DEFAULT_GZIP_LEVEL: u32 = 6;
const DEFAULT_DEFLATE_LEVEL: u32 = 6;
const DEFAULT_BROTLI_LEVEL: u32 = 6;
const DEFAULT_BROTLI_WINDOW: u32 = 22;
const DEFAULT_ZSTD_LEVEL: i32 = 3;
const DEFAULT_MIN_LENGTH: usize = 20;
const DEFAULT_BUFFER_SIZE: usize = 8_192;

/// Location configuration for the compress module. Fields are private; the
/// setters and accessors below are the crate-internal surface. style:allow-pub-crate
#[derive(Debug, Default)]
pub(crate) struct CompressConfig {
    enable: Option<bool>,
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
    // (count, size) of per-request output buffers; only `size` is used today.
    buffers: Option<(usize, usize)>,
    // Pool-allocated array of allowed MIME types; `None` falls back to the
    // built-in compressible set. Copy pointer, owned by the config pool.
    types: Option<NonNull<ngx_array_t>>,
}

/// A resolved, defaults-applied snapshot used on the request path. Each codec
/// field is `Some(params)` when that coding is enabled, else `None`. Public
/// fields keep the request-path access allocation- and accessor-free.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Resolved {
    // style:allow-pub-crate
    pub enabled: bool,
    pub min_length: usize,
    pub vary: bool,
    pub buffer_size: usize,
    pub types: Option<NonNull<ngx_array_t>>,
    pub gzip: Option<u32>,
    pub deflate: Option<u32>,
    pub brotli: Option<(u32, u32)>,
    pub zstd: Option<i32>,
}

impl CompressConfig {
    /// Resolves inheritance and defaults into an immutable snapshot.
    pub(crate) fn resolve(&self) -> Resolved {
        // style:allow-pub-crate
        Resolved {
            enabled: self.enable.unwrap_or(false),
            min_length: self.min_length.unwrap_or(DEFAULT_MIN_LENGTH),
            vary: self.vary.unwrap_or(true),
            buffer_size: self.buffers.map_or(DEFAULT_BUFFER_SIZE, |(_, size)| size),
            types: self.types,
            gzip: on_level(self.gzip, self.gzip_level, DEFAULT_GZIP_LEVEL),
            deflate: on_level(self.deflate, self.deflate_level, DEFAULT_DEFLATE_LEVEL),
            brotli: self.brotli.unwrap_or(false).then(|| {
                (
                    self.brotli_level.unwrap_or(DEFAULT_BROTLI_LEVEL),
                    self.brotli_window.unwrap_or(DEFAULT_BROTLI_WINDOW),
                )
            }),
            zstd: self
                .zstd
                .unwrap_or(false)
                .then(|| self.zstd_level.unwrap_or(DEFAULT_ZSTD_LEVEL)),
        }
    }
}

fn on_level(on: Option<bool>, level: Option<u32>, default: u32) -> Option<u32> {
    on.unwrap_or(false).then(|| level.unwrap_or(default))
}

impl Merge for CompressConfig {
    fn merge(&mut self, prev: &Self) -> Result<(), MergeConfigError> {
        merge_opt(&mut self.enable, prev.enable);
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
        merge_opt(&mut self.types, prev.types);
        Ok(())
    }
}

fn merge_opt<T: Copy>(child: &mut Option<T>, parent: Option<T>) {
    if child.is_none() {
        *child = parent;
    }
}

/// Data-driven setter shared by every `compress_*` directive; dispatches on the
/// directive name in `args[0]`. Referenced by the command table. style:allow-pub-crate
pub(crate) extern "C" fn set_directive(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx::ffi::ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    // SAFETY: nginx passes a valid cf and a pointer to our CompressConfig; every
    // directive is NGX_CONF_TAKE1, so args[0] (name) and args[1] (value) exist.
    unsafe {
        let config = &mut *conf.cast::<CompressConfig>();
        let args: &[ngx_str_t] = (*(*cf).args).as_slice(); // style:allow-explicit-type
        let (Ok(name), Ok(value)) = (args[0].to_str(), args[1].to_str()) else {
            ngx_conf_log_error!(NGX_LOG_EMERG, cf, "compress directive value is not UTF-8");
            return NGX_CONF_ERROR;
        };
        if apply(config, name, value) {
            NGX_CONF_OK
        } else {
            ngx_conf_log_error!(NGX_LOG_EMERG, cf, "invalid value for compress directive");
            NGX_CONF_ERROR
        }
    }
}

fn apply(config: &mut CompressConfig, name: &str, value: &str) -> bool {
    match name {
        "compress" => set_flag(&mut config.enable, value),
        "compress_gzip" => set_flag(&mut config.gzip, value),
        "compress_deflate" => set_flag(&mut config.deflate, value),
        "compress_brotli" => set_flag(&mut config.brotli, value),
        "compress_zstd" => set_flag(&mut config.zstd, value),
        "compress_vary" => set_flag(&mut config.vary, value),
        "compress_gzip_comp_level" => set_u32(&mut config.gzip_level, value, 1, 9),
        "compress_deflate_comp_level" => set_u32(&mut config.deflate_level, value, 1, 9),
        "compress_brotli_comp_level" => set_u32(&mut config.brotli_level, value, 0, 11),
        "compress_brotli_window" => set_u32(&mut config.brotli_window, value, 10, 24),
        "compress_zstd_comp_level" => set_zstd_level(&mut config.zstd_level, value),
        "compress_min_length" => set_usize(&mut config.min_length, value),
        _ => false,
    }
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

/// `compress_buffers <count> <size>` (`NGX_CONF_TAKE2`). style:allow-pub-crate
pub(crate) extern "C" fn set_buffers(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    // SAFETY: nginx passes a valid cf and our CompressConfig; TAKE2 guarantees
    // args[1] (count) and args[2] (size) exist.
    unsafe {
        let config = &mut *conf.cast::<CompressConfig>();
        let args: &[ngx_str_t] = (*(*cf).args).as_slice(); // style:allow-explicit-type
        let mut size_arg = args[2];
        let size = ngx_parse_size(&raw mut size_arg);
        match (
            args[1].to_str().ok().and_then(|c| c.parse::<usize>().ok()),
            size,
        ) {
            (Some(count), size) if count > 0 && size > 0 => {
                config.buffers =
                    Some((count, usize::try_from(size).unwrap_or(DEFAULT_BUFFER_SIZE)));
                NGX_CONF_OK
            }
            _ => {
                ngx_conf_log_error!(NGX_LOG_EMERG, cf, "invalid compress_buffers value");
                NGX_CONF_ERROR
            }
        }
    }
}

/// `compress_types <mime>...` (`NGX_CONF_1MORE`). style:allow-pub-crate
pub(crate) extern "C" fn set_types(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    // SAFETY: nginx passes a valid cf and our CompressConfig; builds a pool-owned
    // array of the MIME arguments and copies each ngx_str into it.
    unsafe {
        let config = &mut *conf.cast::<CompressConfig>();
        let args: &[ngx_str_t] = (*(*cf).args).as_slice(); // style:allow-explicit-type
        let array = ngx_array_create((*cf).pool, args.len() - 1, size_of::<ngx_str_t>());
        if array.is_null() {
            return NGX_CONF_ERROR;
        }
        for mime in &args[1..] {
            // style:allow-for-in
            let slot = ngx_array_push(array).cast::<ngx_str_t>();
            if slot.is_null() {
                return NGX_CONF_ERROR;
            }
            *slot = *mime;
        }
        config.types = NonNull::new(array);
        NGX_CONF_OK
    }
}
