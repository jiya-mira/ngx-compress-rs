//! Request-time enforcement of built-in gzip conflicts.

use std::sync::atomic::Ordering;

use ngx::ffi::{NGX_LOG_WARN, ngx_http_request_t};
use ngx::http::HttpModuleMainConf;
use ngx::ngx_log_error;

use crate::{BuiltinGzip, CompressConfig, DisabledReason, MainConfig, Module, disabled_reason};

/// Returns a fail-closed reason when built-in gzip conflicts with runtime
/// compression. A sidecar-only location (`compress off`) is intentionally not
/// a conflict.
///
/// # Safety
///
/// `request` must remain live for the duration of this prefetch and log step.
impl BuiltinGzip for Module {
    // SAFETY: caller must pass the live request and its effective config.
    unsafe fn disabled_for_request(
        request: *mut ngx_http_request_t,
        config: &CompressConfig,
        runtime_compression_enabled: bool,
    ) -> Option<DisabledReason> {
        if request.is_null() || !runtime_compression_enabled {
            return None;
        }
        // SAFETY: request belongs to the current cycle and MainConfig is our exact
        // module main-conf type.
        let descriptor =
            unsafe { Module::main_conf(&*request) }.and_then(|main: &MainConfig| main.builtin_gzip);
        // SAFETY: descriptor and request come from the same live cycle.
        let state = descriptor.and_then(|flag| unsafe {
            ngx_compress_ffi::module_conf::builtin_gzip_for_request(request, flag)
        });
        let runtime_conflict = state.is_some_and(|state| state.enabled);
        let mismatch = descriptor.is_some() && runtime_conflict != config.gzip_conflict_expected;
        if mismatch && !config.gzip_runtime_warned.swap(true, Ordering::Relaxed) {
            // SAFETY: nginx supplied a live request with a live connection logger.
            let log = unsafe { (*(*request).connection).log };
            ngx_log_error!(
                NGX_LOG_WARN,
                log,
                "module=ngx_compress callback=request_prefetch class=builtin_gzip_state_mismatch"
            );
        }

        // A configuration-time conflict remains disabled if a later runtime read
        // unexpectedly fails or disagrees. Conversely, a newly observed runtime
        // conflict always disables the module.
        disabled_reason(
            runtime_compression_enabled,
            Some(ngx_compress_ffi::module_conf::BuiltinGzipState {
                enabled: runtime_conflict || config.gzip_conflict_expected,
            }),
        )
    }
}
