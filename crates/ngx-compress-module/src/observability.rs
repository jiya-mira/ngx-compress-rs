//! Stable, payload-free failure events emitted at NGINX boundaries.

use ngx::ffi::{NGX_LOG_ERR, ngx_conf_t, ngx_http_request_t};
use ngx::{ngx_conf_log_error, ngx_log_error};

#[derive(Clone, Copy)]
pub enum Callback {
    Preconfiguration,
    Postconfiguration,
    SetDirective,
    SetBuffers,
    SetTypes,
    HeaderFilter,
    BodyFilter,
    StaticHandler,
}

impl Callback {
    const fn name(self) -> &'static str {
        match self {
            Self::Preconfiguration => "preconfiguration",
            Self::Postconfiguration => "postconfiguration",
            Self::SetDirective => "set_directive",
            Self::SetBuffers => "set_buffers",
            Self::SetTypes => "set_types",
            Self::HeaderFilter => "header_filter",
            Self::BodyFilter => "body_filter",
            Self::StaticHandler => "static_handler",
        }
    }
}

#[derive(Clone, Copy)]
pub enum FailureClass {
    RustPanic,
    CodecInitialization,
    CodecReset,
    OutputAllocation,
    InvalidFfiState,
    InvalidCodecProgress,
    CodecBackend,
    Downstream,
}

impl FailureClass {
    const fn name(self) -> &'static str {
        match self {
            Self::RustPanic => "rust_panic",
            Self::CodecInitialization => "codec_initialization",
            Self::CodecReset => "codec_reset",
            Self::OutputAllocation => "output_allocation",
            Self::InvalidFfiState => "invalid_ffi_state",
            Self::InvalidCodecProgress => "invalid_codec_progress",
            Self::CodecBackend => "codec_backend",
            Self::Downstream => "downstream",
        }
    }
}

/// Logs only stable classification keys; never panic payload or request data.
// SAFETY: request must reference a live NGINX request and connection logger.
pub unsafe fn request(request: *mut ngx_http_request_t, callback: Callback, class: FailureClass) {
    if request.is_null() {
        return;
    }
    // SAFETY: caller guarantees the live request and connection.
    let connection = unsafe { (*request).connection };
    if connection.is_null() {
        return;
    }
    // SAFETY: connection owns a live request logger.
    let log = unsafe { (*connection).log };
    if log.is_null() {
        return;
    }
    ngx_log_error!(
        NGX_LOG_ERR,
        log,
        "module=ngx_compress callback={} class={}",
        callback.name(),
        class.name()
    );
}

/// Configuration equivalent of [`request`], with the same payload-free shape.
// SAFETY: cf must reference the live NGINX configuration parser.
pub unsafe fn config(cf: *mut ngx_conf_t, callback: Callback, class: FailureClass) {
    if cf.is_null() {
        return;
    }
    ngx_conf_log_error!(
        NGX_LOG_ERR,
        cf,
        "module=ngx_compress callback={} class={}",
        callback.name(),
        class.name()
    );
}
