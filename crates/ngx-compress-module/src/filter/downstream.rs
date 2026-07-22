//! Downstream filter submission with stable failure logging.

use ngx::core::Status;
use ngx::ffi::{ngx_chain_t, ngx_http_request_t, ngx_int_t};

use crate::fault::{self, Point};
use crate::observability::{self, Callback, FailureClass};

// SAFETY: request must be live and filter installation complete.
pub unsafe fn header(request: *mut ngx_http_request_t) -> ngx_int_t {
    if fault::take(Point::Downstream) {
        // SAFETY: caller guarantees request remains live.
        unsafe {
            observability::request(request, Callback::HeaderFilter, FailureClass::Downstream);
        }
        return Status::NGX_ERROR.0;
    }
    // SAFETY: caller guarantees request and installed chain.
    let rc = unsafe { ngx_compress_ffi::filter::next_header(request) };
    if rc == Status::NGX_ERROR.0 {
        // SAFETY: request remains live after the synchronous downstream call.
        unsafe {
            observability::request(request, Callback::HeaderFilter, FailureClass::Downstream);
        }
    }
    rc
}

// SAFETY: request/chain must be live and filter installation complete.
pub unsafe fn body(request: *mut ngx_http_request_t, chain: *mut ngx_chain_t) -> ngx_int_t {
    // SAFETY: caller guarantees request/chain and installed filter chain.
    let rc = unsafe { ngx_compress_ffi::filter::next_body(request, chain) };
    if rc == Status::NGX_ERROR.0 {
        // SAFETY: request remains live after the synchronous downstream call.
        unsafe {
            observability::request(request, Callback::BodyFilter, FailureClass::Downstream);
        }
    }
    rc
}
