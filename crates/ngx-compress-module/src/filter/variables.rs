//! NGINX variable registration and late `Server-Timing` trailer emission.

use core::ptr;

use ngx::core::Status;
use ngx::ffi::{
    NGX_HTTP_VAR_NOCACHEABLE, ngx_conf_t, ngx_http_add_variable, ngx_http_request_t,
    ngx_http_variable_value_t, ngx_int_t, ngx_list_push, ngx_pnalloc, ngx_str_t, ngx_table_elt_t,
};
use ngx_compress_core::StatsField;

use super::RequestCtx;
use crate::{Module, StatsRegistration, registration::ngx_http_compress_module};

const VARIABLES: [(&str, StatsField); 6] = [
    ("compress_coding", StatsField::Coding),
    ("compress_level", StatsField::Level),
    ("compress_input_bytes", StatsField::InputBytes),
    ("compress_output_bytes", StatsField::OutputBytes),
    ("compress_ratio", StatsField::Ratio),
    ("compress_time_ms", StatsField::TimeMs),
];

impl StatsRegistration for Module {
    unsafe fn register_variables(cf: *mut ngx_conf_t) -> Result<(), ()> {
        for (index, (name, _)) in VARIABLES.iter().enumerate() {
            let mut raw_name = owned_name(name);
            // SAFETY: preconfiguration supplies a live conf and name for the duration
            // of registration; NGINX copies the name into cycle-owned storage.
            let variable = unsafe {
                ngx_http_add_variable(cf, &raw mut raw_name, NGX_HTTP_VAR_NOCACHEABLE as _)
            };
            if variable.is_null() {
                return Err(());
            }
            // SAFETY: add_variable returned a live cycle-owned descriptor.
            unsafe {
                (*variable).get_handler = Some(get_variable);
                (*variable).data = index;
            }
        }
        Ok(())
    }
}

unsafe extern "C" fn get_variable(
    request: *mut ngx_http_request_t,
    value: *mut ngx_http_variable_value_t,
    data: usize,
) -> ngx_int_t {
    if request.is_null() || value.is_null() {
        return Status::NGX_ERROR.0;
    }
    let Some((_, field)) = VARIABLES.get(data) else {
        return Status::NGX_ERROR.0;
    };
    // SAFETY: request owns a live module context table for this callback.
    let raw = unsafe {
        let index = (*ptr::addr_of!(ngx_http_compress_module)).ctx_index;
        (*(*request).ctx.add(index)).cast::<RequestCtx>()
    };
    // SAFETY: value is live and exclusively supplied to this evaluator.
    let output = unsafe { &mut *value };
    if raw.is_null() {
        output.set_not_found(1);
        return Status::NGX_OK.0;
    }
    // SAFETY: the request owns this context until pool cleanup; this callback
    // only reads the statistics snapshot.
    let Some(text) = (unsafe { (*raw).stats }).and_then(|stats| stats.value(*field)) else {
        output.set_not_found(1);
        return Status::NGX_OK.0;
    };
    // SAFETY: request pool remains live until this variable value is discarded.
    let Some(data) = (unsafe { copy_to_pool(request, text.as_bytes()) }) else {
        return Status::NGX_ERROR.0;
    };
    output.set_len(u32::try_from(text.len()).unwrap_or(u32::MAX));
    output.set_valid(1);
    output.set_no_cacheable(1);
    output.set_not_found(0);
    output.data = data;
    Status::NGX_OK.0
}

pub(in crate::filter) unsafe fn add_server_timing(
    request: *mut ngx_http_request_t,
    ctx: &RequestCtx,
) -> Result<(), ()> {
    let stats = ctx.stats.ok_or(())?;
    let timing = stats.server_timing();
    // SAFETY: request owns an initialized output trailer list.
    let header = unsafe {
        ngx_list_push(&raw mut (*request).headers_out.trailers).cast::<ngx_table_elt_t>()
    };
    if header.is_null() {
        return Err(());
    }
    // SAFETY: both allocations belong to the live request pool.
    let key = unsafe { copy_to_pool(request, b"Server-Timing") }.ok_or(())?;
    let value = unsafe { copy_to_pool(request, timing.as_bytes()) }.ok_or(())?;
    // SAFETY: ngx_list_push returned one exclusively owned element.
    unsafe {
        (*header).hash = 1;
        (*header).key = ngx_str_t { len: 13, data: key };
        (*header).value = ngx_str_t {
            len: timing.len(),
            data: value,
        };
    }
    Ok(())
}

fn owned_name(name: &str) -> ngx_str_t {
    ngx_str_t {
        len: name.len(),
        data: name.as_ptr().cast_mut(),
    }
}

unsafe fn copy_to_pool(request: *mut ngx_http_request_t, bytes: &[u8]) -> Option<*mut u8> {
    if bytes.is_empty() {
        return Some(ptr::null_mut());
    }
    // SAFETY: request owns a live pool and the returned allocation is at least
    // bytes.len() writable bytes.
    let output = unsafe { ngx_pnalloc((*request).pool, bytes.len()).cast::<u8>() };
    if output.is_null() {
        return None;
    }
    // SAFETY: source and request-pool destination are valid and non-overlapping.
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len()) };
    Some(output)
}
