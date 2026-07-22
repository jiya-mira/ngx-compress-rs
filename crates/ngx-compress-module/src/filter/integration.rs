//! Filter-chain installation and Accept-Encoding prefetch.

use ngx::ffi::ngx_http_request_t;
use ngx::http::Request;
use ngx_compress_core::AcceptEncoding;

use crate::{FilterModule, Module};

use super::RuntimeCallbacks;

impl FilterModule for Module {
    // SAFETY: registration calls this once during single-threaded postconfiguration.
    unsafe fn install_filters() {
        // SAFETY: postconfiguration owns filter-chain installation.
        unsafe {
            ngx_compress_ffi::filter::install(
                Some(Module::header_filter),
                Some(Module::body_filter),
            );
        }
    }

    /// Uses the public list because NGINX omits its Accept-Encoding convenience
    /// field when built with `--without-http_gzip_module`.
    // SAFETY: `request` must reference a live NGINX request.
    unsafe fn accept_encoding(request: *mut ngx_http_request_t) -> AcceptEncoding {
        // SAFETY: caller guarantees the request remains live during iteration.
        let request = unsafe { Request::from_ngx_http_request(request) };
        let value = request
            .headers_in_iterator()
            .filter(|(name, _)| name.as_ref().eq_ignore_ascii_case(b"accept-encoding"))
            .filter_map(|(_, value)| value.to_str().ok())
            .fold(String::new(), |mut combined, value| {
                if !combined.is_empty() {
                    combined.push(',');
                }
                combined.push_str(value);
                combined
            });
        if value.is_empty() {
            AcceptEncoding::absent()
        } else {
            AcceptEncoding::parse(&value)
        }
    }
}
