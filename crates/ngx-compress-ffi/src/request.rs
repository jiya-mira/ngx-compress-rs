//! Request mutations whose C ABI cannot be represented reliably by bindgen.

use ngx::ffi::ngx_http_request_t;

unsafe extern "C" {
    fn ngx_compress_prepare_encoded_response(request: *mut ngx_http_request_t);
}

/// Prepares response metadata before an output filter changes its bytes.
///
/// # Safety
///
/// `request` must point to the live NGINX request being filtered.
pub unsafe fn prepare_encoded_response(request: *mut ngx_http_request_t) {
    // SAFETY: the caller supplies the live request. The C shim performs the
    // same mutations as NGINX's built-in gzip header filter, including the
    // request bitfield write using the compiler that built NGINX itself.
    unsafe { ngx_compress_prepare_encoded_response(request) }
}
