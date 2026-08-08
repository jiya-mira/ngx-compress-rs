//! NGINX event and connection-bitfield boundary.

use ngx::ffi::ngx_http_request_t;

unsafe extern "C" {
    fn ngx_compress_set_buffered(request: *mut ngx_http_request_t, pending: usize);
    fn ngx_compress_post_write_if_ready(request: *mut ngx_http_request_t);
}

/// Sets or clears this filter's standard HTTP buffered bit while preserving all
/// other connection flags.
///
/// # Safety
///
/// `request` must point to the live request being filtered.
pub unsafe fn set_buffered(request: *mut ngx_http_request_t, pending: bool) {
    // SAFETY: caller supplies a live request. C accesses the connection
    // bitfield using the same compiler and ABI as NGINX.
    unsafe { ngx_compress_set_buffered(request, usize::from(pending)) }
}

/// Defers the connection write event when no downstream connection buffer owns
/// wakeup scheduling.
///
/// This wraps `ngx_post_event(write, &ngx_posted_next_events)`, a C macro. The
/// next-iteration queue preserves the per-callback work bound; unlike the HTTP
/// posted-request queue it is not drained recursively in the current event.
///
/// # Safety
///
/// `request` must point to a live request on its worker event-loop thread.
pub unsafe fn post_write_if_ready(request: *mut ngx_http_request_t) {
    // SAFETY: caller supplies the live request on the worker thread. C reads
    // NGINX bitfields and expands the event macro with the native ABI.
    unsafe { ngx_compress_post_write_if_ready(request) }
}
