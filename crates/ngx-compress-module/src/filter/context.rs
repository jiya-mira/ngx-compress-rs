//! Request-context installation, scoped borrowing, and cleanup.

use core::ffi::c_void;

use ngx::ffi::{ngx_http_request_t, ngx_pool_cleanup_add};
use ngx::http::{HttpModule, Request};
use ngx_compress_core::StreamingCodec;

use super::worker;
use super::{CodecKey, RequestCtx};
use crate::Module;

unsafe extern "C" fn cleanup(data: *mut c_void) {
    ngx_compress_ffi::guard::callback((), || {
        if !data.is_null() {
            // SAFETY: `data` is the RequestCtx pointer allocated in install_ctx.
            let ctx = *unsafe { Box::from_raw(data.cast::<RequestCtx>()) };
            // Return the codec to this worker's pool for reuse; `reset` on the next
            // acquire clears its state. The raw chain pointers are pool-owned and
            // need no drop.
            worker::release(ctx.key, ctx.codec);
        }
    });
}

// SAFETY: `request` must be valid; invoked once per request by the header filter.
pub(in crate::filter) unsafe fn install_ctx(
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
            out: core::ptr::null_mut(),
            busy: core::ptr::null_mut(),
            free: core::ptr::null_mut(),
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

// SAFETY: `request` must own a live module context, uniquely borrowed for the callback.
pub(in crate::filter) unsafe fn with_request_ctx<R>(
    request: *mut ngx_http_request_t,
    use_ctx: impl for<'ctx> FnOnce(&'ctx mut RequestCtx) -> R,
) -> Option<R> {
    // SAFETY: nginx owns a valid ctx table for this request. The higher-ranked
    // callback keeps the unique borrow scoped here and prevents it escaping in R.
    unsafe {
        let index = Module::module().ctx_index;
        let raw = (*(*request).ctx.add(index)).cast::<RequestCtx>();
        if raw.is_null() {
            return None;
        }
        let ctx = &mut *raw;
        if ctx.done { None } else { Some(use_ctx(ctx)) }
    }
}
