//! Header/body filters negotiate a coding, then stream with free/busy-chain backpressure.

mod buffer;
mod context;
mod header;
mod input;
mod runtime;
mod select;
mod worker;

use ngx::ffi::{ngx_buf_t, ngx_chain_t, ngx_http_request_t, ngx_int_t};
use ngx_compress_core::{
    AcceptEncoding, CodecError, ContentCoding, Operation, ResponseFacts, StreamingCodec,
};

use crate::{FilterModule, Module, Resolved};

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

struct InputBuffer<'a> {
    raw: &'a mut ngx_buf_t,
    bytes: &'a [u8],
    operation: Operation,
}

trait RuntimeCallbacks {
    // SAFETY: NGINX must supply a live request pointer.
    unsafe extern "C" fn header_filter(request: *mut ngx_http_request_t) -> ngx_int_t;
    // SAFETY: NGINX must supply a live request and input chain.
    unsafe extern "C" fn body_filter(
        request: *mut ngx_http_request_t,
        chain: *mut ngx_chain_t,
    ) -> ngx_int_t;
}
trait RequestContext {
    // SAFETY: `request` must be live and have no existing module context.
    unsafe fn install(
        request: *mut ngx_http_request_t,
        codec: Box<dyn StreamingCodec>,
        key: CodecKey,
        buffer_size: usize,
    ) -> Option<()>;
    // SAFETY: `request` must own a live, uniquely callback-borrowed context.
    unsafe fn with<R>(
        request: *mut ngx_http_request_t,
        use_ctx: impl for<'ctx> FnOnce(&'ctx mut RequestCtx) -> R,
    ) -> Option<R>;
}

trait CompressChain {
    // SAFETY: request and chain must be live while `self` is uniquely borrowed.
    unsafe fn compress(
        &mut self,
        request: *mut ngx_http_request_t,
        chain: *mut ngx_chain_t,
    ) -> Result<(), ()>;
}

trait InputView<'a>: Sized {
    // SAFETY: non-empty `pos..last` must be one live allocation for `'a`.
    unsafe fn new(raw: &'a mut ngx_buf_t) -> Result<Self, ()>;
    fn operation(&self) -> Operation;
    fn bytes(&self) -> &[u8];
    fn consume(self);
}

trait HeaderDecision: Sized {
    fn decide(resolved: &Resolved<'_>, snapshot: &Snapshot) -> Option<Self>;
}

trait CodecSelection {
    fn choose(
        resolved: &Resolved<'_>,
        accept: &AcceptEncoding,
    ) -> Option<(Box<dyn StreamingCodec>, CodecKey)>;
}

trait CodecPool: Sized {
    fn acquire(self) -> Result<Option<Box<dyn StreamingCodec>>, CodecError>;
    fn release(self, codec: Box<dyn StreamingCodec>);
}

impl FilterModule for Module {
    // SAFETY: registration calls this once during single-threaded postconfiguration.
    unsafe fn install_filters() {
        // SAFETY: postconfiguration owns filter-chain installation.
        unsafe {
            ngx_compress_ffi::filter::install(
                Some(Module::header_filter),
                Some(Module::body_filter),
            );
        }
    }

    /// Copies and parses the request's Accept-Encoding value into Rust-owned state.
    // SAFETY: `request` must reference a live NGINX request.
    unsafe fn accept_encoding(request: *mut ngx_http_request_t) -> AcceptEncoding {
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
}
