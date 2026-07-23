//! Precompressed-sidecar serving (`compress_static`).
//!
//! The safe policy chooses ordered candidates from owned request facts; the
//! [`sidecar`] submit layer maps, opens, and emits one candidate through NGINX.

mod sidecar;

use core::ptr;

use ngx::core::Status;
use ngx::ffi::{
    NGX_HTTP_GET, NGX_HTTP_HEAD, ngx_array_push, ngx_conf_t, ngx_http_conf_ctx_t,
    ngx_http_core_main_conf_t, ngx_http_core_module, ngx_http_handler_pt,
    ngx_http_phases_NGX_HTTP_CONTENT_PHASE, ngx_http_request_t, ngx_int_t, ngx_uint_t,
};
use ngx::http::{HttpModuleLocationConf, Request};
use ngx_compress_core::{StaticMode, StaticRequestFacts, static_candidates};

use crate::observability::{self, Callback, FailureClass};
use crate::{BuiltinGzip, FilterModule, Module, ResolveConfig, StaticModule};

const DECLINED: ngx_int_t = Status::NGX_DECLINED.0;
const OK: ngx_int_t = Status::NGX_OK.0;
const ERROR: ngx_int_t = Status::NGX_ERROR.0;

/// Registers the content-phase handler; call from postconfiguration.
///
/// # Safety
///
/// `cf` must be the valid `ngx_conf_t` nginx passes to postconfiguration.
impl StaticModule for Module {
    unsafe fn register_static(cf: *mut ngx_conf_t) -> Result<(), ()> {
        // SAFETY: caller contract documented above; runs once, single-threaded.
        unsafe {
            let ctx = (*cf).ctx.cast::<ngx_http_conf_ctx_t>();
            let core_index = (*ptr::addr_of!(ngx_http_core_module)).ctx_index;
            let cmcf = (*(*ctx).main_conf.add(core_index)).cast::<ngx_http_core_main_conf_t>();
            let phase = ngx_http_phases_NGX_HTTP_CONTENT_PHASE as usize; // style:allow-as-cast
            let slot = ngx_array_push(&raw mut (*cmcf).phases[phase].handlers)
                .cast::<ngx_http_handler_pt>();
            if slot.is_null() {
                return Err(());
            }
            *slot = Some(handler);
            Ok(())
        }
    }
}

/// Content-phase entry point.
unsafe extern "C" fn handler(request: *mut ngx_http_request_t) -> ngx_int_t {
    ngx_compress_ffi::guard::callback(
        ERROR,
        || {
            // SAFETY: nginx supplied the live request to this callback.
            unsafe {
                observability::request(request, Callback::StaticHandler, FailureClass::RustPanic);
            }
        },
        || {
            // SAFETY: nginx passes a valid request to a content-phase handler.
            unsafe { serve(request) }
        },
    )
}

// SAFETY: `request` must remain valid for the content-phase callback.
unsafe fn serve(request: *mut ngx_http_request_t) -> ngx_int_t {
    // SAFETY: scope the official wrapper to reading configuration; the resolved
    // snapshot borrows only configuration-owned MIME data.
    let config = unsafe {
        let req = Request::from_ngx_http_request(request);
        Module::location_conf(req)
    };
    let Some(config) = config else {
        return DECLINED;
    };
    let resolved = config.resolve();
    if resolved.static_mode == StaticMode::Off {
        return DECLINED;
    }
    // Built-in gzip conflicts only when runtime compression is enabled. This
    // deliberately preserves `compress off; compress_static on;` locations.
    // SAFETY: nginx supplied the live request and configuration.
    if unsafe { Module::disabled_for_request(request, config, resolved.enabled) }.is_some() {
        return DECLINED;
    }
    // SAFETY: copy the method, URI, and Accept-Encoding into Rust-owned facts.
    let Some(snapshot) = (unsafe { prefetch_request(request) }) else {
        return DECLINED;
    };

    let candidates = static_candidates(resolved.static_mode, &snapshot);
    // Only `on` selects a representation from Accept-Encoding. `always` sends
    // the same highest-priority sidecar to every client and therefore does not vary.
    let vary = resolved.static_mode == StaticMode::On && resolved.vary;
    // SAFETY: submit layer probes the complete candidate set and may emit one response.
    unsafe { sidecar::probe_and_serve(request, candidates, vary) }
}

/// Copies all static-sidecar policy inputs out of nginx request memory.
// SAFETY: `request` must reference a live NGINX request and URI.
unsafe fn prefetch_request(request: *mut ngx_http_request_t) -> Option<StaticRequestFacts> {
    // SAFETY: nginx supplied a valid request and a live URI ngx_str.
    unsafe {
        let get_head = (NGX_HTTP_GET | NGX_HTTP_HEAD) as ngx_uint_t;
        Some(StaticRequestFacts {
            method_supported: (*request).method & get_head != 0,
            uri: ngx_compress_ffi::string::copy_bytes(&(*request).uri)?,
            accept_encoding: Module::accept_encoding(request),
        })
    }
}
