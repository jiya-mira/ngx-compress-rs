use core::ptr;

use ngx::core::Status;
use ngx::ffi::{NGX_HTTP_OK, ngx_chain_t, ngx_chain_update_chains, ngx_http_request_t, ngx_int_t};
use ngx::http::{HttpModuleLocationConf, Request};
use ngx_compress_core::ResponseFacts;
use ngx_compress_ffi::filter;

use crate::registration::ngx_http_compress_module;
use crate::{CompressConfig, FilterModule, Module, ResolveConfig};

use super::{
    CompressChain, HeaderDecision, Plan, RequestContext, RequestCtx, RuntimeCallbacks, Snapshot,
};

impl RuntimeCallbacks for Module {
    // Installed into the header filter chain by the parent filter module.
    unsafe extern "C" fn header_filter(request: *mut ngx_http_request_t) -> ngx_int_t {
        ngx_compress_ffi::guard::callback(Status::NGX_ERROR.0, || {
            // SAFETY: nginx supplied the request pointer to this callback.
            unsafe { header_filter_inner(request) }
        })
    }

    // Installed into the body filter chain by the parent filter module.
    unsafe extern "C" fn body_filter(
        request: *mut ngx_http_request_t,
        chain: *mut ngx_chain_t,
    ) -> ngx_int_t {
        ngx_compress_ffi::guard::callback(Status::NGX_ERROR.0, || {
            // SAFETY: nginx supplied both pointers to this callback.
            unsafe { body_filter_inner(request, chain) }
        })
    }
}

unsafe fn header_filter_inner(request: *mut ngx_http_request_t) -> ngx_int_t {
    // SAFETY: install() ran during postconfiguration; used for every fall-through.
    let pass = || unsafe { filter::next_header(request) };

    // SAFETY: scope the official request wrapper to configuration lookup. The
    // resolved MIME reference is configuration-owned, not request-owned.
    let resolved = unsafe {
        let req = Request::from_ngx_http_request(request);
        Module::location_conf(req).map(CompressConfig::resolve)
    };
    let Some(resolved) = resolved else {
        return pass();
    };
    if !resolved.enabled {
        return pass();
    }
    // SAFETY: copy all request/response facts needed by policy into Rust-owned
    // values before safe-core decision making.
    let Some(snapshot) = (unsafe { prefetch_header(request) }) else {
        return pass();
    };
    let Some(plan) = Plan::decide(&resolved, &snapshot) else {
        return pass();
    };

    // SAFETY: create the request wrapper only for the submit phase.
    let req = unsafe { Request::from_ngx_http_request(request) };
    // Adding Vary first makes a later Content-Encoding allocation failure safe
    // to pass through: an extra Vary is harmless, while a lone encoding is not.
    if plan.vary && req.add_header_out("Vary", "Accept-Encoding").is_none() {
        return pass();
    }
    if req
        .add_header_out("Content-Encoding", plan.coding.as_str())
        .is_none()
    {
        return pass();
    }
    // SAFETY: require in-memory input (materialize file buffers first, like the
    // gzip module), clear the invalid length, and install request state.
    unsafe {
        (*request).set_main_filter_need_in_memory(1);
        clear_content_length(request);
        if RequestCtx::install(request, plan.codec, plan.key, plan.buffer_size).is_none() {
            return Status::NGX_ERROR.0;
        }
        filter::next_header(request)
    }
}

// SAFETY: nginx must supply a valid request and input chain for this callback.
unsafe fn body_filter_inner(
    request: *mut ngx_http_request_t,
    chain: *mut ngx_chain_t,
) -> ngx_int_t {
    // SAFETY: scopes the unique ctx borrow to this callback invocation so it
    // cannot escape with a lifetime chosen by the caller.
    let run = |ctx: &mut RequestCtx| {
        // SAFETY: nginx supplied `request` and `chain`; `ctx` is uniquely
        // borrowed for the duration of this callback.
        unsafe { body_filter_with_ctx(request, chain, ctx) }
    };
    let Some(rc) = (unsafe { RequestCtx::with(request, run) }) else {
        // SAFETY: install() ran during postconfiguration.
        return unsafe { filter::next_body(request, chain) };
    };
    rc
}

/// Runs the body filter with a callback-scoped request-context borrow.
// SAFETY: request and chain must be valid while ctx is uniquely callback-borrowed.
unsafe fn body_filter_with_ctx(
    request: *mut ngx_http_request_t,
    chain: *mut ngx_chain_t,
    ctx: &mut RequestCtx,
) -> ngx_int_t {
    // SAFETY: walks the input chain, feeding each buffer to the codec.
    if unsafe { ctx.compress(request, chain) }.is_err() {
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

/// Copies policy inputs out of nginx before safe-core decision making.
unsafe fn prefetch_header(request: *mut ngx_http_request_t) -> Option<Snapshot> {
    // SAFETY: nginx supplied a valid request and live input/output headers.
    unsafe {
        let length = (*request).headers_out.content_length_n;
        let content_length = if length < 0 {
            None
        } else {
            Some(usize::try_from(length).ok()?)
        };
        Some(Snapshot {
            facts: ResponseFacts {
                main_response: request == (*request).main,
                successful: u32::try_from((*request).headers_out.status).ok()? == NGX_HTTP_OK,
                already_encoded: !(*request).headers_out.content_encoding.is_null(),
                content_length,
                content_type: ngx_compress_ffi::string::copy_string(
                    &(*request).headers_out.content_type,
                )?,
            },
            accept_encoding: Module::accept_encoding(request),
        })
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
