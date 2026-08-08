//! Internal contracts shared by the safe policy, runtime, and FFI adapters.

use ngx::ffi::{ngx_buf_t, ngx_chain_t, ngx_http_request_t, ngx_int_t};
use ngx_compress_core::{AcceptEncoding, CodecError, Operation, StreamingCodec};

use super::{
    CodecKey, CodecSelectionFailure, CompressionFailure, RequestCtx, SelectedCodec, Snapshot,
};
use crate::{Resolved, StatsMode};

pub(in crate::filter) trait RuntimeCallbacks {
    // SAFETY: NGINX must supply a live request pointer.
    unsafe extern "C" fn header_filter(request: *mut ngx_http_request_t) -> ngx_int_t;
    // SAFETY: NGINX must supply a live request and input chain.
    unsafe extern "C" fn body_filter(
        request: *mut ngx_http_request_t,
        chain: *mut ngx_chain_t,
    ) -> ngx_int_t;
}

pub(in crate::filter) trait RequestContext {
    // SAFETY: `request` must be live and have no existing module context.
    unsafe fn install(
        request: *mut ngx_http_request_t,
        codec: Box<dyn StreamingCodec>,
        key: CodecKey,
        buffer_size: usize,
        stats_mode: StatsMode,
        buffer_count: usize,
    ) -> Option<()>;
    // SAFETY: `request` must own a live, uniquely callback-borrowed context.
    unsafe fn with<R>(
        request: *mut ngx_http_request_t,
        use_ctx: impl for<'ctx> FnOnce(&'ctx mut RequestCtx) -> R,
    ) -> Option<R>;
}

pub(in crate::filter) trait CompressChain {
    // SAFETY: request and chain must be live while `self` is uniquely borrowed.
    unsafe fn compress(
        &mut self,
        request: *mut ngx_http_request_t,
        chain: *mut ngx_chain_t,
    ) -> Result<bool, CompressionFailure>;
}

pub(in crate::filter) trait InputView<'a>: Sized {
    // SAFETY: non-empty `pos..last` must be one live allocation for `'a`.
    unsafe fn new(raw: &'a mut ngx_buf_t) -> Result<Self, ()>;
    fn operation(&self) -> Operation;
    fn bytes(&self) -> &[u8];
    fn consume(self, bytes: usize) -> Result<bool, ()>;
}

pub(in crate::filter) trait HeaderDecision: Sized {
    fn decide(
        resolved: &Resolved<'_>,
        snapshot: &Snapshot,
    ) -> Result<Option<Self>, CodecSelectionFailure>;
}

pub(in crate::filter) trait CodecSelection {
    fn choose(
        resolved: &Resolved<'_>,
        accept: &AcceptEncoding,
    ) -> Result<Option<SelectedCodec>, CodecSelectionFailure>;
}

pub(in crate::filter) trait CodecPool: Sized {
    fn acquire(self) -> Result<Option<Box<dyn StreamingCodec>>, CodecError>;
    fn release(self, codec: Box<dyn StreamingCodec>);
}
