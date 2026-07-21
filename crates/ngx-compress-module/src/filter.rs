// style:allow-file-size (nginx body-filter buffer plumbing is cohesive here)
//! Header and body output filters: negotiate a coding, then stream the response
//! body through the selected codec with free/busy chain backpressure.

use core::ffi::c_void;
use core::ptr::NonNull;
use core::{ptr, slice, str};

use ngx::core::Status;
use ngx::ffi::{
    NGX_HTTP_OK, ngx_array_t, ngx_buf_t, ngx_chain_get_free_buf, ngx_chain_t,
    ngx_chain_update_chains, ngx_http_request_t, ngx_int_t, ngx_palloc, ngx_pool_cleanup_add,
    ngx_str_t,
};
use ngx::http::{HttpModule, HttpModuleLocationConf, Request};
use ngx_compress_core::{AcceptEncoding, Operation, StepState, StreamingCodec, checked_step};
use ngx_compress_ffi::filter;

use crate::conf::{CompressConfig, Resolved};
use crate::registration::{Module, ngx_http_compress_module};
use crate::select;
use crate::worker::{self, CodecKey};

/// Per-request compression state, owned via the request `ctx` slot and dropped
/// by a registered pool cleanup so the codec's resources are released.
struct RequestCtx {
    codec: Box<dyn StreamingCodec>,
    // Identifies the codec for return to the worker pool at cleanup.
    key: CodecKey,
    out: *mut ngx_chain_t,
    busy: *mut ngx_chain_t,
    free: *mut ngx_chain_t,
    buffer_size: usize,
    done: bool,
}

// Installed into the header filter chain by registration. style:allow-pub-crate
pub(crate) unsafe extern "C" fn header_filter(request: *mut ngx_http_request_t) -> ngx_int_t {
    // SAFETY: nginx passes a valid request pointer to a header filter.
    let req = unsafe { Request::from_ngx_http_request(request) };
    // SAFETY: install() ran during postconfiguration; used for every fall-through.
    let pass = || unsafe { filter::next_header(request) };

    let Some(resolved) = Module::location_conf(req).map(CompressConfig::resolve) else {
        return pass();
    };
    // SAFETY: reads response/request header fields from a valid request.
    if !resolved.enabled || !unsafe { eligible(request, &resolved) } {
        return pass();
    }

    // SAFETY: reads the request Accept-Encoding header.
    let accept = unsafe { accept_encoding(request) };
    let Some((codec, key)) = select::choose(&resolved, &accept) else {
        return pass();
    };
    let coding = codec.coding();

    if req
        .add_header_out("Content-Encoding", coding.as_str())
        .is_none()
        || (resolved.vary && req.add_header_out("Vary", "Accept-Encoding").is_none())
    {
        return pass();
    }
    // SAFETY: require in-memory input (materialize file buffers first, like the
    // gzip module), clear the invalid length, and install request state.
    unsafe {
        (*request).set_main_filter_need_in_memory(1);
        clear_content_length(request);
        if install_ctx(request, codec, key, resolved.buffer_size).is_none() {
            return Status::NGX_ERROR.0;
        }
        filter::next_header(request)
    }
}

// Installed into the body filter chain by registration. style:allow-pub-crate
pub(crate) unsafe extern "C" fn body_filter(
    request: *mut ngx_http_request_t,
    chain: *mut ngx_chain_t,
) -> ngx_int_t {
    // SAFETY: reads the module ctx slot for this request.
    let Some(ctx) = (unsafe { request_ctx(request) }) else {
        // SAFETY: install() ran during postconfiguration.
        return unsafe { filter::next_body(request, chain) };
    };

    // SAFETY: walks the input chain, feeding each buffer to the codec.
    if unsafe { compress_chain(request, ctx, chain) }.is_err() {
        return Status::NGX_ERROR.0;
    }

    // Nothing produced yet and nothing pending: the codec buffered this input.
    // Return OK without invoking the next filter, or it would see an empty chain.
    if ctx.out.is_null() && ctx.busy.is_null() {
        return Status::NGX_OK.0;
    }

    // SAFETY: forwards the produced output chain and reclaims consumed buffers.
    let rc = unsafe { filter::next_body(request, ctx.out) };
    // SAFETY: moves fully-sent buffers from busy to free and resets `out`.
    unsafe {
        ngx_chain_update_chains(
            (*request).pool,
            &raw mut ctx.free,
            &raw mut ctx.busy,
            &raw mut ctx.out,
            ptr::addr_of!(ngx_http_compress_module).cast_mut().cast(),
        );
    }
    rc
}

/// Drives the codec over the whole input chain, appending output buffers to
/// `ctx.out`. Marks each input buffer consumed as it is accepted.
///
/// # Safety
///
/// `request` and `chain` must be valid, and `ctx` uniquely borrowed.
// SAFETY: caller upholds the contract in the `# Safety` section above.
unsafe fn compress_chain(
    request: *mut ngx_http_request_t,
    ctx: &mut RequestCtx,
    mut chain: *mut ngx_chain_t,
) -> Result<(), ()> {
    while !chain.is_null() {
        // SAFETY: `chain` is a valid link; read its buffer and input region.
        let (buf, operation, input) = unsafe {
            let buf = (*chain).buf;
            (buf, operation_for(buf), input_slice(buf))
        };
        let mut offset = 0;

        loop {
            // SAFETY: reuse or allocate an output buffer link from the pool.
            let link = unsafe { free_buf(request, ctx) };
            if link.is_null() {
                return Err(());
            }
            // SAFETY: `link`'s buffer is valid; `writable` is its [last, end) tail.
            let (out_buf, capacity) = unsafe {
                let out_buf = (*link).buf;
                (out_buf, writable(out_buf))
            };
            let step = checked_step(&mut *ctx.codec, operation, &input[offset..], capacity)
                .map_err(|_| ())?;
            offset += step.consumed;

            let complete = step.state == StepState::Complete;
            let last = operation == Operation::Finish && complete;
            let flush = operation == Operation::Flush && complete;

            // Only emit a buffer that carries data or a boundary flag; an empty
            // non-boundary buffer (a codec step that buffered input without
            // output) would trip nginx's "zero size buf" alert.
            if step.produced > 0 || last || flush {
                // SAFETY: commit produced bytes, flag boundaries, link into `out`.
                unsafe {
                    advance(out_buf, step.produced);
                    if last {
                        (*out_buf).set_last_buf(1);
                    } else if flush {
                        (*out_buf).set_flush(1);
                    }
                    append(&mut ctx.out, link);
                }
            } else {
                // SAFETY: return the unused empty buffer to the free list.
                unsafe { recycle(&mut ctx.free, link) };
            }

            if last {
                ctx.done = true;
                break;
            }
            if flush || step.state == StepState::NeedsInput {
                break;
            }
        }

        // SAFETY: input fully consumed; advance to the next chain link.
        unsafe {
            (*buf).pos = (*buf).last;
            chain = (*chain).next;
        }
    }
    Ok(())
}

/// Chooses the codec operation for an input buffer from its nginx flags.
unsafe fn operation_for(buf: *mut ngx_buf_t) -> Operation {
    // SAFETY: `buf` is a valid buffer pointer; read its boundary flags.
    unsafe {
        if (*buf).last_buf() != 0 {
            Operation::Finish
        } else if (*buf).flush() != 0 || (*buf).last_in_chain() != 0 {
            Operation::Flush
        } else {
            Operation::Continue
        }
    }
}

unsafe fn input_slice<'a>(buf: *mut ngx_buf_t) -> &'a [u8] {
    // SAFETY: pos..last is the readable region; empty when pos == last.
    unsafe {
        let len = usize::try_from((*buf).last.offset_from((*buf).pos)).unwrap_or(0);
        slice::from_raw_parts((*buf).pos, len)
    }
}

unsafe fn writable<'a>(buf: *mut ngx_buf_t) -> &'a mut [u8] {
    // SAFETY: last..end is the unused writable region of a temp buffer.
    unsafe {
        let len = usize::try_from((*buf).end.offset_from((*buf).last)).unwrap_or(0);
        slice::from_raw_parts_mut((*buf).last, len)
    }
}

unsafe fn advance(buf: *mut ngx_buf_t, produced: usize) {
    // SAFETY: `produced` bytes were just written after `last`, within `end`.
    unsafe { (*buf).last = (*buf).last.add(produced) };
}

unsafe fn append(out: &mut *mut ngx_chain_t, link: *mut ngx_chain_t) {
    // SAFETY: append `link` to the tail of the `out` chain.
    unsafe {
        (*link).next = ptr::null_mut();
        let mut tail = out;
        while !(*tail).is_null() {
            tail = &mut (**tail).next;
        }
        *tail = link;
    }
}

unsafe fn recycle(free: &mut *mut ngx_chain_t, link: *mut ngx_chain_t) {
    // SAFETY: prepend an unused buffer link back onto the free chain for reuse.
    unsafe {
        (*link).next = *free;
        *free = link;
    }
}

/// Reuses a buffer from the free chain, or allocates a new temp buffer + link.
unsafe fn free_buf(request: *mut ngx_http_request_t, ctx: &mut RequestCtx) -> *mut ngx_chain_t {
    // SAFETY: pool is valid; ngx_chain_get_free_buf reuses or allocates a link.
    unsafe {
        let pool = (*request).pool;
        let link = ngx_chain_get_free_buf(pool, &raw mut ctx.free);
        if link.is_null() {
            return ptr::null_mut();
        }
        let buf = (*link).buf;
        if (*buf).start.is_null() {
            let memory = ngx_palloc(pool, ctx.buffer_size).cast::<u8>();
            if memory.is_null() {
                return ptr::null_mut();
            }
            (*buf).start = memory;
            (*buf).end = memory.add(ctx.buffer_size);
            (*buf).tag = ptr::addr_of!(ngx_http_compress_module).cast_mut().cast();
        }
        (*buf).pos = (*buf).start;
        (*buf).last = (*buf).start;
        (*buf).set_temporary(1);
        link
    }
}

/// Returns whether the response is eligible for compression.
unsafe fn eligible(request: *mut ngx_http_request_t, resolved: &Resolved) -> bool {
    // SAFETY: reads response header fields from a valid request.
    unsafe {
        if request != (*request).main {
            return false;
        }
        if u32::try_from((*request).headers_out.status).unwrap_or(0) != NGX_HTTP_OK {
            return false;
        }
        if !(*request).headers_out.content_encoding.is_null() {
            return false;
        }
        let length = (*request).headers_out.content_length_n;
        if length >= 0 && usize::try_from(length).unwrap_or(0) < resolved.min_length {
            return false;
        }
        compressible(&(*request).headers_out.content_type, resolved.types)
    }
}

fn compressible(content_type: &ngx_str_t, types: Option<NonNull<ngx_array_t>>) -> bool {
    // SAFETY: content_type.data points to len valid bytes (or is empty).
    let bytes = unsafe { slice::from_raw_parts(content_type.data, content_type.len) };
    let head = bytes.split(|&b| b == b';').next().unwrap_or(bytes);
    let Ok(kind) = str::from_utf8(head) else {
        return false;
    };
    let kind = kind.trim();
    match types {
        // SAFETY: `array` is a pool-owned ngx_array_t of ngx_str_t entries.
        Some(array) => unsafe { type_in_list(kind, array) },
        None => builtin_compressible(kind),
    }
}

fn builtin_compressible(kind: &str) -> bool {
    kind.starts_with("text/")
        || matches!(
            kind,
            "application/json"
                | "application/javascript"
                | "application/xml"
                | "application/rss+xml"
                | "application/atom+xml"
                | "application/wasm"
                | "image/svg+xml"
        )
}

unsafe fn type_in_list(kind: &str, array: NonNull<ngx_array_t>) -> bool {
    // SAFETY: iterate the pool-owned array of ngx_str_t MIME entries.
    unsafe {
        let array = array.as_ptr();
        let entries = (*array).elts.cast::<ngx_str_t>();
        (0..(*array).nelts).any(|index| {
            let entry = &*entries.add(index);
            let bytes = slice::from_raw_parts(entry.data, entry.len);
            bytes == b"*" || str::from_utf8(bytes).is_ok_and(|mime| mime.eq_ignore_ascii_case(kind))
        })
    }
}

// Shared with the static-sidecar handler. style:allow-pub-crate
pub(crate) unsafe fn accept_encoding(request: *mut ngx_http_request_t) -> AcceptEncoding {
    // SAFETY: reads the request Accept-Encoding header element if present.
    unsafe {
        let header = (*request).headers_in.accept_encoding;
        if header.is_null() {
            return AcceptEncoding::absent();
        }
        let value = (*header).value;
        match str::from_utf8(slice::from_raw_parts(value.data, value.len)) {
            Ok(text) => AcceptEncoding::parse(text),
            Err(_) => AcceptEncoding::absent(),
        }
    }
}

unsafe fn clear_content_length(request: *mut ngx_http_request_t) {
    // SAFETY: removes the length so nginx re-frames the compressed body.
    unsafe {
        (*request).headers_out.content_length_n = -1;
        let length = (*request).headers_out.content_length;
        if !length.is_null() {
            (*length).hash = 0;
            (*request).headers_out.content_length = ptr::null_mut();
        }
    }
}

unsafe extern "C" fn cleanup(data: *mut c_void) {
    if !data.is_null() {
        // SAFETY: `data` is the RequestCtx pointer we leaked in install_ctx.
        let ctx = *unsafe { Box::from_raw(data.cast::<RequestCtx>()) };
        // Return the codec to this worker's pool for reuse; `reset` on the next
        // acquire clears its state. The raw chain pointers are pool-owned and
        // need no drop.
        worker::release(ctx.key, ctx.codec);
    }
}

// SAFETY: `request` must be valid; invoked once per request by the header filter.
unsafe fn install_ctx(
    request: *mut ngx_http_request_t,
    codec: Box<dyn StreamingCodec>,
    key: CodecKey,
    buffer_size: usize,
) -> Option<()> {
    // SAFETY: allocates a cleanup handler tied to the request pool.
    unsafe {
        let cleanup_handler = ngx_pool_cleanup_add((*request).pool, 0);
        if cleanup_handler.is_null() {
            return None;
        }
        let boxed = Box::into_raw(Box::new(RequestCtx {
            codec,
            key,
            out: ptr::null_mut(),
            busy: ptr::null_mut(),
            free: ptr::null_mut(),
            buffer_size,
            done: false,
        }));
        (*cleanup_handler).handler = Some(cleanup);
        (*cleanup_handler).data = boxed.cast();
        let req = Request::from_ngx_http_request(request);
        req.set_module_ctx(boxed.cast(), Module::module());
        Some(())
    }
}

unsafe fn request_ctx<'a>(request: *mut ngx_http_request_t) -> Option<&'a mut RequestCtx> {
    // SAFETY: read the ctx slot as a raw pointer (never via &T), so &mut is
    // sound — single-threaded request, sole owner until pool cleanup.
    unsafe {
        let index = Module::module().ctx_index;
        let raw = (*(*request).ctx.add(index)).cast::<RequestCtx>();
        if raw.is_null() {
            return None;
        }
        let ctx = &mut *raw;
        if ctx.done { None } else { Some(ctx) }
    }
}
