//! Header/body filters negotiate a coding, then stream with free/busy-chain backpressure.

mod buffer;
mod context;
mod downstream;
mod header;
mod input;
mod integration;
mod runtime;
mod select;
mod variables;
mod worker;

use ngx::ffi::{ngx_buf_t, ngx_chain_t, ngx_http_request_t, ngx_int_t};
use ngx_compress_core::{
    AcceptEncoding, CodecError, CompressionStats, ContentCoding, Operation, ResponseFacts,
    StreamingCodec,
};

use crate::{Resolved, StatsMode};

struct RequestCtx {
    codec: Box<dyn StreamingCodec>,
    key: CodecKey,
    out: *mut ngx_chain_t,
    busy: *mut ngx_chain_t,
    free: *mut ngx_chain_t,
    input: *mut ngx_chain_t,
    buffer_size: usize,
    buffer_count: usize,
    allocated_buffers: usize,
    pending_operation: Option<Operation>,
    done: bool,
    stats: Option<CompressionStats>,
    server_timing: bool,
    trailer_sent: bool,
}

struct Snapshot {
    facts: ResponseFacts,
    accept_encoding: AcceptEncoding,
}

struct Plan {
    codec: Box<dyn StreamingCodec>,
    key: CodecKey,
    coding: ContentCoding,
    vary: bool,
    buffer_size: usize,
    stats_mode: StatsMode,
    buffer_count: usize,
    reset_recovered: bool,
}

struct SelectedCodec {
    codec: Box<dyn StreamingCodec>,
    key: CodecKey,
    reset_recovered: bool,
}

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

#[derive(Clone, Copy)]
enum OutputFailure {
    Allocation,
    Exhausted,
    InvalidFfiState,
}

#[derive(Clone, Copy)]
enum CompressionFailure {
    OutputAllocation,
    InvalidFfiState,
    InvalidCodecProgress,
    CodecBackend,
}

#[derive(Clone, Copy)]
enum CodecSelectionFailure {
    Initialization,
    NotAcceptable,
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
        stats_mode: StatsMode,
        buffer_count: usize,
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
    ) -> Result<bool, CompressionFailure>;
}

trait InputView<'a>: Sized {
    // SAFETY: non-empty `pos..last` must be one live allocation for `'a`.
    unsafe fn new(raw: &'a mut ngx_buf_t) -> Result<Self, ()>;
    fn operation(&self) -> Operation;
    fn bytes(&self) -> &[u8];
    fn consume(self, bytes: usize) -> Result<bool, ()>;
}

trait HeaderDecision: Sized {
    fn decide(
        resolved: &Resolved<'_>,
        snapshot: &Snapshot,
    ) -> Result<Option<Self>, CodecSelectionFailure>;
}

trait CodecSelection {
    fn choose(
        resolved: &Resolved<'_>,
        accept: &AcceptEncoding,
    ) -> Result<Option<SelectedCodec>, CodecSelectionFailure>;
}

trait CodecPool: Sized {
    fn acquire(self) -> Result<Option<Box<dyn StreamingCodec>>, CodecError>;
    fn release(self, codec: Box<dyn StreamingCodec>);
}
