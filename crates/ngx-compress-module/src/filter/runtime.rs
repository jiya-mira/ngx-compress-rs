use core::ptr;

use ngx::core::Status;
use ngx::ffi::{NGX_HTTP_OK, ngx_chain_t, ngx_chain_update_chains, ngx_http_request_t, ngx_int_t};
use ngx::http::{HttpModuleLocationConf, Request};
use ngx_compress_core::ResponseFacts;

use crate::fault::{self, Point};
use crate::observability::{self, Callback, FailureClass};
use crate::registration::ngx_http_compress_module;
use crate::{BuiltinGzip, FilterModule, Module, ResolveConfig};

use super::{
    CodecSelectionFailure, CompressChain, CompressionFailure, HeaderDecision, Plan, RequestContext,
    RequestCtx, RuntimeCallbacks, Snapshot,
};

const NOT_ACCEPTABLE: ngx_int_t = 406;

impl RuntimeCallbacks for Module {
    // Installed into the header filter chain by the parent filter module.
    unsafe extern "C" fn header_filter(request: *mut ngx_http_request_t) -> ngx_int_t {
        ngx_compress_ffi::guard::callback(
            Status::NGX_ERROR.0,
            || {
                // SAFETY: nginx supplied the live request to this callback.
                unsafe {
                    observability::request(
                        request,
                        Callback::HeaderFilter,
                        FailureClass::RustPanic,
                    );
                }
            },
            || {
                // SAFETY: nginx supplied the request pointer to this callback.
                unsafe { header_filter_inner(request) }
            },
        )
    }

    // Installed into the body filter chain by the parent filter module.
    unsafe extern "C" fn body_filter(
        request: *mut ngx_http_request_t,
        chain: *mut ngx_chain_t,
    ) -> ngx_int_t {
        ngx_compress_ffi::guard::callback(
            Status::NGX_ERROR.0,
            || {
                // SAFETY: nginx supplied the live request to this callback.
                unsafe {
                    observability::request(request, Callback::BodyFilter, FailureClass::RustPanic);
                }
            },
            || {
                // SAFETY: nginx supplied both pointers to this callback.
                unsafe { body_filter_inner(request, chain) }
            },
        )
    }
}

unsafe fn header_filter_inner(request: *mut ngx_http_request_t) -> ngx_int_t {
    // SAFETY: install() ran during postconfiguration; used for every fall-through.
    let pass = || unsafe { super::downstream::header(request) };

    // SAFETY: scope the official request wrapper to configuration lookup. The
    // resolved MIME reference is configuration-owned, not request-owned.
    let config = unsafe {
        let req = Request::from_ngx_http_request(request);
        Module::location_conf(req)
    };
    let Some(config) = config else {
        return pass();
    };
    let resolved = config.resolve();
    if !resolved.enabled {
        return pass();
    }
    // SAFETY: copy built-in gzip state before safe-core conflict enforcement.
    if unsafe { Module::disabled_for_request(request, config, resolved.enabled) }.is_some() {
        return pass();
    }
    // SAFETY: copy all request/response facts needed by policy into Rust-owned
    // values before safe-core decision making.
    let Some(snapshot) = (unsafe { prefetch_header(request) }) else {
        return pass();
    };
    let plan = match Plan::decide(&resolved, &snapshot) {
        Ok(Some(plan)) => plan,
        Ok(None) => return pass(),
        Err(CodecSelectionFailure::NotAcceptable) => {
            if resolved.vary {
                // Preserve cache correctness because 406 depends on Accept-Encoding.
                let req = unsafe { Request::from_ngx_http_request(request) };
                let _ = req.add_header_out("Vary", "Accept-Encoding");
            }
            return NOT_ACCEPTABLE;
        }
        Err(CodecSelectionFailure::Initialization) => {
            // SAFETY: request remains live and no response headers were changed.
            unsafe {
                observability::request(
                    request,
                    Callback::HeaderFilter,
                    FailureClass::CodecInitialization,
                );
            }
            return pass();
        }
    };
    if plan.reset_recovered {
        // SAFETY: request remains live and a fresh codec recovered the request.
        unsafe {
            observability::request(request, Callback::HeaderFilter, FailureClass::CodecReset);
        }
    }

    // SAFETY: create the request wrapper only for the submit phase.
    let req = unsafe { Request::from_ngx_http_request(request) };
    if fault::take(Point::HeaderAllocation) {
        // SAFETY: request remains live and no response headers were changed.
        unsafe {
            observability::request(
                request,
                Callback::HeaderFilter,
                FailureClass::OutputAllocation,
            );
        }
        return pass();
    }
    // Adding Vary first makes a later Content-Encoding allocation failure safe
    // to pass through: an extra Vary is harmless, while a lone encoding is not.
    if plan.vary && req.add_header_out("Vary", "Accept-Encoding").is_none() {
        // SAFETY: request remains live in this header callback.
        unsafe {
            observability::request(
                request,
                Callback::HeaderFilter,
                FailureClass::OutputAllocation,
            );
        }
        return pass();
    }
    if req
        .add_header_out("Content-Encoding", plan.coding.as_str())
        .is_none()
    {
        // SAFETY: request remains live in this header callback.
        unsafe {
            observability::request(
                request,
                Callback::HeaderFilter,
                FailureClass::OutputAllocation,
            );
        }
        return pass();
    }
    // SAFETY: require in-memory input (materialize file buffers first, like the
    // gzip module), clear representation-specific metadata, and install request
    // state before the downstream header filters observe the response.
    unsafe {
        ngx_compress_ffi::request::prepare_encoded_response(request);
        if plan.stats_mode == crate::StatsMode::ServerTiming {
            (*request).set_expect_trailers(1);
        }
        if RequestCtx::install(
            request,
            plan.codec,
            plan.key,
            plan.buffer_size,
            plan.stats_mode,
        )
        .is_none()
        {
            observability::request(
                request,
                Callback::HeaderFilter,
                FailureClass::OutputAllocation,
            );
            return Status::NGX_ERROR.0;
        }
        super::downstream::header(request)
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
        return unsafe { super::downstream::body(request, chain) };
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
    if let Err(error) = unsafe { ctx.compress(request, chain) } {
        let class = match error {
            CompressionFailure::OutputAllocation => FailureClass::OutputAllocation,
            CompressionFailure::InvalidFfiState => FailureClass::InvalidFfiState,
            CompressionFailure::InvalidCodecProgress => FailureClass::InvalidCodecProgress,
            CompressionFailure::CodecBackend => FailureClass::CodecBackend,
        };
        // SAFETY: request remains live in this body callback.
        unsafe { observability::request(request, Callback::BodyFilter, class) };
        return Status::NGX_ERROR.0;
    }

    if ctx.done && ctx.server_timing && !ctx.trailer_sent {
        // SAFETY: request and its output trailer list remain live for this callback.
        if unsafe { super::variables::add_server_timing(request, ctx) }.is_err() {
            unsafe {
                observability::request(
                    request,
                    Callback::BodyFilter,
                    FailureClass::OutputAllocation,
                );
            }
        }
        ctx.trailer_sent = true;
    }

    // Nothing produced yet and nothing pending: the codec buffered this input.
    // Return OK without invoking the next filter, or it would see an empty chain.
    if ctx.out.is_null() && ctx.busy.is_null() {
        return Status::NGX_OK.0;
    }

    // SAFETY: forwards the produced output chain and reclaims consumed buffers.
    let rc = unsafe { super::downstream::body(request, ctx.out) };
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
