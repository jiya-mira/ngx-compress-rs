//! Module identity and postconfiguration callback.

use core::ptr;

use ngx::core::Status;
use ngx::ffi::{ngx_conf_t, ngx_int_t, ngx_module_t};
use ngx::http::HttpModule;

use crate::observability::{self, Callback, FailureClass};
use crate::{BuiltinGzipRegistration, FilterModule, Module, StaticModule};

use super::ngx_http_compress_module;

impl HttpModule for Module {
    fn module() -> &'static ngx_module_t {
        // SAFETY: the module static is initialized at load time and never moved.
        unsafe { &*ptr::addr_of!(ngx_http_compress_module) }
    }

    unsafe extern "C" fn postconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
        ngx_compress_ffi::guard::callback(
            Status::NGX_ERROR.0,
            || {
                // SAFETY: nginx supplied the live configuration pointer.
                unsafe {
                    observability::config(cf, Callback::Postconfiguration, FailureClass::RustPanic);
                }
            },
            || {
                // SAFETY: nginx supplied the configuration pointer while still
                // single-threaded.
                unsafe { postconfiguration_inner(cf) }
            },
        )
    }
}

// SAFETY: cf must be the live NGINX postconfiguration pointer.
unsafe fn postconfiguration_inner(cf: *mut ngx_conf_t) -> ngx_int_t {
    // SAFETY: postconfiguration owns the live cycle and all merged HTTP confs.
    if unsafe { Module::discover_gzip_and_warn(cf) }.is_err() {
        // SAFETY: cf remains live for this callback.
        unsafe {
            observability::config(
                cf,
                Callback::Postconfiguration,
                FailureClass::InvalidFfiState,
            );
        }
        return Status::NGX_ERROR.0;
    }
    // SAFETY: runs once in the single-threaded master before workers fork.
    unsafe {
        Module::install_filters();
        if Module::register_static(cf).is_err() {
            observability::config(
                cf,
                Callback::Postconfiguration,
                FailureClass::OutputAllocation,
            );
            return Status::NGX_ERROR.0;
        }
    }
    Status::NGX_OK.0
}
