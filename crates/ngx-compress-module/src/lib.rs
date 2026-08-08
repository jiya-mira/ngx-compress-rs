//! NGINX module entrypoint: negotiates `Accept-Encoding`, then streams through
//! the selected codec with free/busy-chain backpressure. Supports static and
//! dynamic NGINX builds.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ngx_compress_core::{ContentCoding, MimeTypes, StaticMode};
use ngx_compress_ffi::module_conf::{BuiltinGzipState, HttpLocFlag};

mod config;
mod fault;
mod filter;
mod gzip;
mod observability;
mod registration;
mod static_file;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Profile {
    Custom,
    Fast,
    Balanced,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StatsMode {
    #[default]
    Off,
    Variables,
    ServerTiming,
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
    stats_mode: Option<StatsMode>,
    types: Option<Arc<MimeTypes>>,
    priority: Option<Arc<[ContentCoding]>>,
    gzip_conflict_expected: bool,
    gzip_runtime_warned: AtomicBool,
}

/// Cycle-owned metadata discovered without depending on gzip's private struct.
#[derive(Debug, Default)]
struct MainConfig {
    builtin_gzip: Option<HttpLocFlag>,
}

/// Defaults-applied, allocation-free request-path configuration snapshot.
#[derive(Clone, Copy, Debug)]
struct Resolved<'a> {
    enabled: bool,
    min_length: usize,
    vary: bool,
    buffer_size: usize,
    stats_mode: StatsMode,
    buffer_count: usize,
    static_mode: StaticMode,
    types: Option<&'a MimeTypes>,
    gzip: Option<u32>,
    deflate: Option<u32>,
    brotli: Option<(u32, u32)>,
    zstd: Option<i32>,
    priority: [ContentCoding; 5],
}

/// Validated, Rust-owned configuration update produced at the FFI boundary.
enum ConfigUpdate {
    Named { name: String, value: String },
    Buffers { count: usize, size: usize },
    Types(Vec<String>),
    Priority(Vec<String>),
}

struct Module;

/// Safe-core reasons why this module must leave a response untouched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DisabledReason {
    BuiltinGzipEnabled,
}

fn disabled_reason(
    runtime_compression_enabled: bool,
    builtin_gzip: Option<BuiltinGzipState>,
) -> Option<DisabledReason> {
    (runtime_compression_enabled && builtin_gzip.is_some_and(|state| state.enabled))
        .then_some(DisabledReason::BuiltinGzipEnabled)
}

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

trait StatsRegistration {
    // SAFETY: `cf` must be the live NGINX preconfiguration pointer.
    unsafe fn register_variables(cf: *mut ngx::ffi::ngx_conf_t) -> Result<(), ()>;
}

trait BuiltinGzip {
    // SAFETY: `request` must reference a live request using `config`.
    unsafe fn disabled_for_request(
        request: *mut ngx::ffi::ngx_http_request_t,
        config: &CompressConfig,
        runtime_compression_enabled: bool,
    ) -> Option<DisabledReason>;
}
trait BuiltinGzipRegistration {
    // SAFETY: `cf` must be the live postconfiguration pointer.
    unsafe fn discover_gzip_and_warn(cf: *mut ngx::ffi::ngx_conf_t) -> Result<(), ()>;
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

    extern "C" fn set_priority(
        cf: *mut ngx::ffi::ngx_conf_t,
        command: *mut ngx::ffi::ngx_command_t,
        conf: *mut core::ffi::c_void,
    ) -> *mut core::ffi::c_char;
}
