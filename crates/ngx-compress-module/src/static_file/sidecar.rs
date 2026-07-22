//! NGINX path mapping, cached-file lookup, and sidecar response submission.

use core::mem::size_of;
use core::ptr;

use ngx::ffi::{
    NGX_HTTP_INTERNAL_SERVER_ERROR, NGX_HTTP_OK, ngx_buf_t, ngx_chain_t, ngx_file_t,
    ngx_http_core_module, ngx_http_discard_request_body, ngx_http_map_uri_to_path,
    ngx_http_output_filter, ngx_http_request_t, ngx_http_send_header, ngx_http_set_content_type,
    ngx_http_set_etag, ngx_int_t, ngx_list_push, ngx_open_cached_file, ngx_open_file_info_t,
    ngx_pcalloc, ngx_str_t, ngx_table_elt_t, ngx_uint_t,
};
use ngx_compress_core::ContentCoding;

use crate::observability::{self, Callback, FailureClass};

use super::{ERROR, OK, SidecarSubmit};

/// Checked mapped-path buffer with room reserved for one sidecar extension.
struct MappedPath(ngx_str_t);

impl MappedPath {
    /// Maps the request URI and appends `extension` inside nginx's reserved tail.
    ///
    /// # Safety
    ///
    /// `request` must be valid. NGINX must honor the documented reserved-tail
    /// contract of `ngx_http_map_uri_to_path`.
    // SAFETY: the caller must uphold the request and reserved-tail contract above.
    unsafe fn with_extension(
        request: *mut ngx_http_request_t,
        extension: &str,
    ) -> Result<Self, ()> {
        let mut path = ngx_str_t {
            len: 0,
            data: ptr::null_mut(),
        };
        let mut root = 0usize;
        // SAFETY: caller supplies a valid request; path/root are live outputs.
        let last = unsafe {
            ngx_http_map_uri_to_path(request, &raw mut path, &raw mut root, extension.len())
        };
        if last.is_null() || path.data.is_null() {
            return Err(());
        }
        let base_len = last.addr().checked_sub(path.data.addr()).ok_or(())?;
        let total_len = base_len.checked_add(extension.len()).ok_or(())?;

        // SAFETY: map_uri_to_path reserved extension.len() bytes plus its NUL
        // terminator beginning at `last`.
        unsafe {
            ptr::copy_nonoverlapping(extension.as_ptr(), last, extension.len());
            *last.add(extension.len()) = 0;
        }
        path.len = total_len;
        Ok(Self(path))
    }

    fn as_mut_ptr(&mut self) -> *mut ngx_str_t {
        &raw mut self.0
    }

    fn into_raw(self) -> ngx_str_t {
        self.0
    }
}

/// Attempts to open and serve the sidecar for one coding. `Some(rc)` means we
/// handled the request; `None` means no such sidecar, so try the next candidate.
// SAFETY: `request` must be a valid NGINX request for the full probe and submit.
impl SidecarSubmit for ngx_compress_core::StaticCandidate {
    unsafe fn try_serve(self, request: *mut ngx_http_request_t) -> Option<ngx_int_t> {
        // SAFETY: builds the sidecar path, opens it via the location's file cache,
        // and, if present, emits the file as the response body.
        unsafe {
            let Ok(mut path) = MappedPath::with_extension(request, self.extension) else {
                return Some(fail(request, FailureClass::InvalidFfiState));
            };

            let clcf = core_loc_conf(request);
            if clcf.is_null() {
                return Some(fail(request, FailureClass::InvalidFfiState));
            }
            let mut of = core::mem::zeroed::<ngx_open_file_info_t>();
            of.read_ahead = (*clcf).read_ahead;
            of.directio = (*clcf).directio;
            of.valid = (*clcf).open_file_cache_valid;
            of.min_uses = (*clcf).open_file_cache_min_uses;

            if ngx_open_cached_file(
                (*clcf).open_file_cache,
                path.as_mut_ptr(),
                &raw mut of,
                (*request).pool,
            ) != OK
                || of.is_dir() != 0
            {
                return None;
            }
            Some(send_file(request, self.coding, path.into_raw(), &of))
        }
    }
}

/// Emits an opened sidecar file as the response body.
// SAFETY: `request` must be valid and `of` must describe a live opened file.
unsafe fn send_file(
    request: *mut ngx_http_request_t,
    coding: ContentCoding,
    path: ngx_str_t,
    of: &ngx_open_file_info_t,
) -> ngx_int_t {
    // SAFETY: `of` describes an open file; set the response headers and stream it.
    unsafe {
        let discard = ngx_http_discard_request_body(request);
        if discard != OK {
            observability::request(request, Callback::StaticHandler, FailureClass::Downstream);
            return discard;
        }

        (*request).headers_out.status = NGX_HTTP_OK as ngx_uint_t;
        (*request).headers_out.content_length_n = of.size;
        (*request).headers_out.last_modified_time = of.mtime;
        if ngx_http_set_content_type(request) != OK || !add_content_encoding(request, coding) {
            return fail(request, FailureClass::OutputAllocation);
        }
        if ngx_http_set_etag(request) != OK {
            return fail(request, FailureClass::OutputAllocation);
        }

        let buf = ngx_pcalloc((*request).pool, size_of::<ngx_buf_t>()).cast::<ngx_buf_t>();
        let file = ngx_pcalloc((*request).pool, size_of::<ngx_file_t>()).cast::<ngx_file_t>();
        if buf.is_null() || file.is_null() {
            return fail(request, FailureClass::OutputAllocation);
        }
        (*file).fd = of.fd;
        (*file).name = path;
        (*file).log = (*(*request).connection).log;
        (*file).set_directio(of.is_directio());
        (*buf).file_pos = 0;
        (*buf).file_last = of.size;
        (*buf).file = file;
        (*buf).set_in_file(1);
        (*buf).set_last_buf(u32::from(request == (*request).main));
        (*buf).set_last_in_chain(1);

        let mut out = ngx_chain_t {
            buf,
            next: ptr::null_mut(),
        };
        let rc = ngx_http_send_header(request);
        if rc == ERROR || rc > OK || (*request).header_only() != 0 {
            if rc == ERROR {
                observability::request(request, Callback::StaticHandler, FailureClass::Downstream);
            }
            return rc;
        }
        let rc = ngx_http_output_filter(request, &raw mut out);
        if rc == ERROR {
            observability::request(request, Callback::StaticHandler, FailureClass::Downstream);
        }
        rc
    }
}

/// Adds `Content-Encoding: <coding>` to the response and records it.
// SAFETY: `request` must own a live, mutable output-header list.
unsafe fn add_content_encoding(request: *mut ngx_http_request_t, coding: ContentCoding) -> bool {
    // SAFETY: pushes a header onto the (uninitialized) headers_out list slot.
    unsafe {
        let elt = ngx_list_push(&raw mut (*request).headers_out.headers).cast::<ngx_table_elt_t>();
        if elt.is_null() {
            return false;
        }
        (*elt).hash = 1;
        (*elt).key = static_str("Content-Encoding");
        (*elt).value = static_str(coding.as_str());
        (*elt).lowcase_key = ptr::null_mut();
        (*elt).next = ptr::null_mut();
        (*request).headers_out.content_encoding = elt;
        true
    }
}

/// The core module's per-location config, holding the open-file cache and roots.
// SAFETY: `request` must have a populated location-configuration array.
unsafe fn core_loc_conf(
    request: *mut ngx_http_request_t,
) -> *mut ngx::ffi::ngx_http_core_loc_conf_t {
    // SAFETY: indexes r->loc_conf by the core module's ctx_index.
    unsafe {
        let index = (*ptr::addr_of!(ngx_http_core_module)).ctx_index;
        (*(*request).loc_conf.add(index)).cast()
    }
}

/// `500 Internal Server Error` as an `ngx_int_t` return code.
fn server_error() -> ngx_int_t {
    ngx_int_t::try_from(NGX_HTTP_INTERNAL_SERVER_ERROR).unwrap_or(ERROR)
}

// SAFETY: request must remain live for logging.
unsafe fn fail(request: *mut ngx_http_request_t, class: FailureClass) -> ngx_int_t {
    // SAFETY: caller guarantees the live request.
    unsafe { observability::request(request, Callback::StaticHandler, class) };
    server_error()
}

/// Wraps a `'static` string as an `ngx_str_t`; its bytes outlive the response.
fn static_str(value: &'static str) -> ngx_str_t {
    ngx_str_t {
        len: value.len(),
        data: value.as_ptr().cast_mut(),
    }
}
