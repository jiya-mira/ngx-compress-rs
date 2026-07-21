//! Header and body output filters: negotiate a coding, then stream the response
//! body through the selected codec with free/busy chain backpressure.

mod buffer;
mod context;
mod header;
mod input;
mod runtime;
mod select;
mod worker;

use ngx::ffi::{ngx_chain_t, ngx_http_request_t};
use ngx_compress_core::{AcceptEncoding, ContentCoding, ResponseFacts, StreamingCodec};

/// Per-request compression state, owned through the request context slot.
struct RequestCtx {
    codec: Box<dyn StreamingCodec>,
    key: CodecKey,
    out: *mut ngx_chain_t,
    busy: *mut ngx_chain_t,
    free: *mut ngx_chain_t,
    buffer_size: usize,
    done: bool,
}

/// Owned values prefetched from one nginx request/response.
struct Snapshot {
    facts: ResponseFacts,
    accept_encoding: AcceptEncoding,
}

/// Complete safe-core decision consumed by the FFI submit layer.
struct Plan {
    codec: Box<dyn StreamingCodec>,
    key: CodecKey,
    coding: ContentCoding,
    vary: bool,
    buffer_size: usize,
}

/// Identifies an interchangeable worker-local codec instance.
#[derive(Clone, Copy, PartialEq, Eq)]
struct CodecKey {
    coding: ContentCoding,
    level: i32,
    window: u32,
}

impl CodecKey {
    fn new(coding: ContentCoding, level: i32, window: u32) -> Self {
        Self {
            coding,
            level,
            window,
        }
    }
}

/// Installs this module's callbacks into the NGINX output-filter chain.
// SAFETY: registration calls this once during single-threaded postconfiguration.
pub(crate) unsafe fn install() {
    // SAFETY: postconfiguration owns filter-chain installation.
    unsafe {
        ngx_compress_ffi::filter::install(Some(runtime::header_filter), Some(runtime::body_filter));
    }
}

/// Copies and parses the request's Accept-Encoding value into Rust-owned state.
// SAFETY: `request` must reference a live NGINX request.
pub(crate) unsafe fn accept_encoding(request: *mut ngx_http_request_t) -> AcceptEncoding {
    // SAFETY: reads the request Accept-Encoding header element if present.
    unsafe {
        let header = (*request).headers_in.accept_encoding;
        if header.is_null() {
            return AcceptEncoding::absent();
        }
        ngx_compress_ffi::string::copy_string(&(*header).value)
            .map_or_else(AcceptEncoding::absent, |text| AcceptEncoding::parse(&text))
    }
}
