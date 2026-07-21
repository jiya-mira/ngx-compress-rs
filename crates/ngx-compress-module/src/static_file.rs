// style:allow-file-size (one cohesive content-phase handler + its FFI plumbing)
//! Precompressed-sidecar serving (`compress_static`).
//!
//! A NGINX content-phase handler, in the spirit of `gzip_static`: for `GET`/`HEAD`
//! it looks for a precompressed sidecar next to the requested file (`x.js.zst`,
//! `x.js.br`, `x.js.gz`) that the client accepts, and serves it directly with the
//! right `Content-Encoding` — no runtime compression. If none exists it declines
//! so the normal static handler serves the original.
//!
//! Serving happens before the output filters run, so our own header filter sees
//! `Content-Encoding` already set and passes the bytes through untouched.

use core::mem::size_of;
use core::ptr;

use ngx::core::Status;
use ngx::ffi::{
    NGX_HTTP_GET, NGX_HTTP_HEAD, NGX_HTTP_INTERNAL_SERVER_ERROR, NGX_HTTP_OK, ngx_array_push,
    ngx_buf_t, ngx_chain_t, ngx_conf_t, ngx_file_t, ngx_http_conf_ctx_t, ngx_http_core_main_conf_t,
    ngx_http_core_module, ngx_http_discard_request_body, ngx_http_handler_pt,
    ngx_http_map_uri_to_path, ngx_http_output_filter, ngx_http_phases_NGX_HTTP_CONTENT_PHASE,
    ngx_http_request_t, ngx_http_send_header, ngx_http_set_content_type, ngx_http_set_etag,
    ngx_int_t, ngx_list_push, ngx_open_cached_file, ngx_open_file_info_t, ngx_pcalloc, ngx_str_t,
    ngx_table_elt_t, ngx_uint_t,
};
use ngx::http::{HttpModuleLocationConf, Request};
use ngx_compress_core::{ContentCoding, StaticCandidate, StaticRequestFacts, static_candidates};

use crate::conf::CompressConfig;
use crate::filter::accept_encoding;
use crate::registration::Module;

const DECLINED: ngx_int_t = Status::NGX_DECLINED.0;
const OK: ngx_int_t = Status::NGX_OK.0;
const ERROR: ngx_int_t = Status::NGX_ERROR.0;

/// `500 Internal Server Error` as an `ngx_int_t` return code.
fn server_error() -> ngx_int_t {
    ngx_int_t::try_from(NGX_HTTP_INTERNAL_SERVER_ERROR).unwrap_or(ERROR)
}

/// Registers the content-phase handler; call from postconfiguration.
///
/// # Safety
///
/// `cf` must be the valid `ngx_conf_t` nginx passes to postconfiguration.
// style:allow-pub-crate
pub(crate) unsafe fn register(cf: *mut ngx_conf_t) -> Result<(), ()> {
    // SAFETY: caller contract documented above; runs once, single-threaded.
    unsafe {
        let ctx = (*cf).ctx.cast::<ngx_http_conf_ctx_t>();
        let core_index = (*ptr::addr_of!(ngx_http_core_module)).ctx_index;
        let cmcf = (*(*ctx).main_conf.add(core_index)).cast::<ngx_http_core_main_conf_t>();
        let phase = ngx_http_phases_NGX_HTTP_CONTENT_PHASE as usize; // style:allow-as-cast
        let slot =
            ngx_array_push(&raw mut (*cmcf).phases[phase].handlers).cast::<ngx_http_handler_pt>();
        if slot.is_null() {
            return Err(());
        }
        *slot = Some(handler);
        Ok(())
    }
}

/// Content-phase entry point.
unsafe extern "C" fn handler(request: *mut ngx_http_request_t) -> ngx_int_t {
    // SAFETY: nginx passes a valid request to a content-phase handler.
    unsafe { serve(request) }
}

unsafe fn serve(request: *mut ngx_http_request_t) -> ngx_int_t {
    // SAFETY: scope the official wrapper to reading configuration; the resolved
    // snapshot borrows only configuration-owned MIME data.
    let resolved = unsafe {
        let req = Request::from_ngx_http_request(request);
        Module::location_conf(req).map(CompressConfig::resolve)
    };
    let Some(resolved) = resolved else {
        return DECLINED;
    };
    // SAFETY: copy the method, URI, and Accept-Encoding into Rust-owned facts.
    let Some(snapshot) = (unsafe { prefetch_request(request) }) else {
        return DECLINED;
    };

    // style:allow-for-in
    for StaticCandidate { coding, extension } in static_candidates(resolved.static_mode, &snapshot)
    {
        // SAFETY: submit layer probes and possibly emits this complete candidate.
        if let Some(rc) = unsafe { try_serve(request, coding, extension) } {
            return rc;
        }
    }
    DECLINED
}

/// Copies all static-sidecar policy inputs out of nginx request memory.
unsafe fn prefetch_request(request: *mut ngx_http_request_t) -> Option<StaticRequestFacts> {
    // SAFETY: nginx supplied a valid request and a live URI ngx_str.
    unsafe {
        let get_head = (NGX_HTTP_GET | NGX_HTTP_HEAD) as ngx_uint_t;
        Some(StaticRequestFacts {
            method_supported: (*request).method & get_head != 0,
            uri: copy_ngx_bytes(&(*request).uri)?,
            accept_encoding: accept_encoding(request),
        })
    }
}

/// Copies an nginx byte string into Rust ownership without assuming UTF-8.
unsafe fn copy_ngx_bytes(value: &ngx_str_t) -> Option<Vec<u8>> {
    if value.len == 0 {
        return Some(Vec::new());
    }
    if value.data.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees that non-empty data is live for len bytes;
    // to_vec removes the external lifetime before policy sees the value.
    Some(unsafe { core::slice::from_raw_parts(value.data, value.len) }.to_vec())
}

/// Attempts to open and serve the sidecar for one coding. `Some(rc)` means we
/// handled the request (served it or hit an error); `None` means no such sidecar
/// — try the next candidate.
// SAFETY: `request` is a valid nginx request; the body upholds the FFI contract.
unsafe fn try_serve(
    request: *mut ngx_http_request_t,
    coding: ContentCoding,
    ext: &str,
) -> Option<ngx_int_t> {
    // SAFETY: builds the sidecar path, opens it via the location's file cache,
    // and, if present, emits the file as the response body.
    unsafe {
        let mut path = ngx_str_t {
            len: 0,
            data: ptr::null_mut(),
        };
        let mut root = 0usize;
        let last = ngx_http_map_uri_to_path(request, &raw mut path, &raw mut root, ext.len());
        if last.is_null() {
            return Some(server_error());
        }
        // `last` reuses the mapped path's NUL slot; append the extension + NUL.
        let bytes = ext.as_bytes();
        ptr::copy_nonoverlapping(bytes.as_ptr(), last, bytes.len());
        *last.add(bytes.len()) = 0;
        path.len = usize::try_from(last.offset_from(path.data)).unwrap_or(0) + bytes.len();

        let clcf = core_loc_conf(request);
        let mut of: ngx_open_file_info_t = core::mem::zeroed();
        of.read_ahead = (*clcf).read_ahead;
        of.directio = (*clcf).directio;
        of.valid = (*clcf).open_file_cache_valid;
        of.min_uses = (*clcf).open_file_cache_min_uses;

        if ngx_open_cached_file(
            (*clcf).open_file_cache,
            &raw mut path,
            &raw mut of,
            (*request).pool,
        ) != OK
            || of.is_dir() != 0
        {
            return None;
        }
        Some(send_file(request, coding, path, &of))
    }
}

/// Emits an opened sidecar file as the response body.
// SAFETY: `request` is valid and `of` describes a file opened by the caller.
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
            return discard;
        }

        (*request).headers_out.status = NGX_HTTP_OK as ngx_uint_t;
        (*request).headers_out.content_length_n = of.size;
        (*request).headers_out.last_modified_time = of.mtime;
        if ngx_http_set_content_type(request) != OK || !add_content_encoding(request, coding) {
            return server_error();
        }
        if ngx_http_set_etag(request) != OK {
            return server_error();
        }

        let buf = ngx_pcalloc((*request).pool, size_of::<ngx_buf_t>()).cast::<ngx_buf_t>();
        let file = ngx_pcalloc((*request).pool, size_of::<ngx_file_t>()).cast::<ngx_file_t>();
        if buf.is_null() || file.is_null() {
            return ERROR;
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
            return rc;
        }
        ngx_http_output_filter(request, &raw mut out)
    }
}

/// Adds `Content-Encoding: <coding>` to the response and records it.
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
// SAFETY: `request` is a valid nginx request with a populated `loc_conf` array.
unsafe fn core_loc_conf(
    request: *mut ngx_http_request_t,
) -> *mut ngx::ffi::ngx_http_core_loc_conf_t {
    // SAFETY: indexes r->loc_conf by the core module's ctx_index.
    unsafe {
        let index = (*ptr::addr_of!(ngx_http_core_module)).ctx_index;
        (*(*request).loc_conf.add(index)).cast()
    }
}

/// Wraps a `'static` string as an `ngx_str_t`; its bytes outlive the response.
fn static_str(value: &'static str) -> ngx_str_t {
    ngx_str_t {
        len: value.len(),
        data: value.as_ptr().cast_mut(),
    }
}
