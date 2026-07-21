//! Insertion of Rust filters into the NGINX output header and body chains.
//!
//! NGINX exposes the top of each output filter chain as a mutable global
//! function pointer. A filter module saves the current top pointer as its
//! "next" filter and installs its own, so control flows through every module in
//! registration order. This module owns that plumbing behind a safe surface.

use core::ptr::{addr_of, addr_of_mut};

use ngx::core::Status;
use ngx::ffi::{
    ngx_chain_t, ngx_http_output_body_filter_pt, ngx_http_output_header_filter_pt,
    ngx_http_request_t, ngx_http_top_body_filter, ngx_http_top_header_filter, ngx_int_t,
};

static mut NEXT_HEADER_FILTER: ngx_http_output_header_filter_pt = None;
static mut NEXT_BODY_FILTER: ngx_http_output_body_filter_pt = None;

/// Installs `header` and `body` at the top of the NGINX output filter chains,
/// remembering the previous top filters for [`next_header`] and [`next_body`].
///
/// # Safety
///
/// Must be called exactly once, from a module's `postconfiguration` callback,
/// while NGINX is still single-threaded. `header` and `body` must remain valid
/// for the lifetime of the process.
// SAFETY: caller contract is documented in the `# Safety` section above.
pub unsafe fn install(
    header: ngx_http_output_header_filter_pt,
    body: ngx_http_output_body_filter_pt,
) {
    // SAFETY: postconfiguration runs once in the single-threaded master before
    // workers fork, so these globals are unraced; save each top before overwrite.
    unsafe {
        addr_of_mut!(NEXT_HEADER_FILTER).write(ngx_http_top_header_filter);
        ngx_http_top_header_filter = header;
        addr_of_mut!(NEXT_BODY_FILTER).write(ngx_http_top_body_filter);
        ngx_http_top_body_filter = body;
    }
}

/// Invokes the next header filter in the chain.
///
/// # Safety
///
/// `request` must be a valid NGINX request pointer, and [`install`] must have
/// run during configuration. Returns `NGX_ERROR` if no next filter was stored.
#[must_use]
pub unsafe fn next_header(request: *mut ngx_http_request_t) -> ngx_int_t {
    // SAFETY: NEXT_HEADER_FILTER was written by install() before any request.
    match unsafe { addr_of!(NEXT_HEADER_FILTER).read() } {
        // SAFETY: the stored value is a valid nginx filter installed by install().
        Some(filter) => unsafe { filter(request) },
        None => Status::NGX_ERROR.0,
    }
}

/// Invokes the next body filter in the chain.
///
/// # Safety
///
/// `request` and `chain` must be valid NGINX pointers, and [`install`] must
/// have run during configuration. Returns `NGX_ERROR` if no next filter was
/// stored.
#[must_use]
pub unsafe fn next_body(request: *mut ngx_http_request_t, chain: *mut ngx_chain_t) -> ngx_int_t {
    // SAFETY: NEXT_BODY_FILTER was written by install() before any request.
    match unsafe { addr_of!(NEXT_BODY_FILTER).read() } {
        // SAFETY: the stored value is a valid nginx filter installed by install().
        Some(filter) => unsafe { filter(request, chain) },
        None => Status::NGX_ERROR.0,
    }
}
