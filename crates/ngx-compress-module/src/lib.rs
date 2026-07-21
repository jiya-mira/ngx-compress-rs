//! NGINX dynamic module entrypoint for `ngx-compress-rs`.
//!
//! The header filter negotiates a content coding from the request's
//! `Accept-Encoding` and the location's `compress_*` configuration; the body
//! filter streams the response through the selected codec with free/busy chain
//! backpressure. Builds as both a static and a dynamic NGINX module.

use std::sync::Arc;

use ngx_compress_core::{MimeTypes, StaticMode};

mod config;
mod filter;
mod registration;
mod static_file;

/// Which named preset the `compress` directive selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Profile {
    Custom,
    Fast,
    Balanced,
    Max,
}

/// Rust-owned location configuration. `None` means inherit from the parent.
#[derive(Debug, Default)]
struct CompressConfig {
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
struct Resolved<'a> {
    enabled: bool,
    min_length: usize,
    vary: bool,
    buffer_size: usize,
    static_mode: StaticMode,
    types: Option<&'a MimeTypes>,
    gzip: Option<u32>,
    deflate: Option<u32>,
    brotli: Option<(u32, u32)>,
    zstd: Option<i32>,
}

/// Validated, Rust-owned configuration update produced at the FFI boundary.
enum ConfigUpdate {
    Named { name: String, value: String },
    Buffers { count: usize, size: usize },
    Types(Vec<String>),
}

/// NGINX module type shared as the configuration and request-context key.
struct Module;

/// Configuration behavior implemented by the safe parsing layer.
trait ApplyConfig {
    fn apply(&mut self, update: ConfigUpdate) -> bool;
}

/// Configuration behavior implemented by the safe resolution layer.
trait ResolveConfig {
    fn resolve(&self) -> Resolved<'_>;
}

/// Filter integration supplied by the filter boundary.
trait FilterModule {
    // SAFETY: may only run during single-threaded NGINX postconfiguration.
    unsafe fn install_filters();
    // SAFETY: `request` must reference a live NGINX request.
    unsafe fn accept_encoding(
        request: *mut ngx::ffi::ngx_http_request_t,
    ) -> ngx_compress_core::AcceptEncoding;
}

/// Static-file integration supplied by the content-handler boundary.
trait StaticModule {
    // SAFETY: `cf` must be the live NGINX postconfiguration pointer.
    unsafe fn register_static(cf: *mut ngx::ffi::ngx_conf_t) -> Result<(), ()>;
}

/// Directive callbacks supplied by the configuration FFI boundary.
trait DirectiveCallbacks {
    extern "C" fn set_directive(
        cf: *mut ngx::ffi::ngx_conf_t,
        command: *mut ngx::ffi::ngx_command_t,
        conf: *mut core::ffi::c_void,
    ) -> *mut core::ffi::c_char;

    extern "C" fn set_buffers(
        cf: *mut ngx::ffi::ngx_conf_t,
        command: *mut ngx::ffi::ngx_command_t,
        conf: *mut core::ffi::c_void,
    ) -> *mut core::ffi::c_char;

    extern "C" fn set_types(
        cf: *mut ngx::ffi::ngx_conf_t,
        command: *mut ngx::ffi::ngx_command_t,
        conf: *mut core::ffi::c_void,
    ) -> *mut core::ffi::c_char;
}
