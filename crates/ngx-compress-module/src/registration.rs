//! The `ngx_http_compress_module` static definition, its directive table, and
//! installation of the header/body filters during postconfiguration.

use core::ptr;

use ngx::core::Status;
use ngx::ffi::{
    NGX_CONF_1MORE, NGX_CONF_TAKE1, NGX_CONF_TAKE2, NGX_HTTP_LOC_CONF, NGX_HTTP_LOC_CONF_OFFSET,
    NGX_HTTP_MAIN_CONF, NGX_HTTP_MODULE, NGX_HTTP_SRV_CONF, ngx_command_t, ngx_conf_t,
    ngx_http_module_t, ngx_int_t, ngx_module_t, ngx_str_t, ngx_uint_t,
};
use ngx::http::{HttpModule, HttpModuleLocationConf};
use ngx::ngx_string;

use crate::conf::{set_buffers, set_directive, set_types};
use crate::config::CompressConfig;
use crate::filter::{body_filter, header_filter};

/// The module type, used by the filters as their config/ctx key. style:allow-pub-crate
pub(crate) struct Module;

impl HttpModule for Module {
    fn module() -> &'static ngx_module_t {
        // SAFETY: the module static is initialized at load time and never moved.
        unsafe { &*ptr::addr_of!(ngx_http_compress_module) }
    }

    unsafe extern "C" fn postconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
        ngx_compress_ffi::guard::callback(Status::NGX_ERROR.0, || {
            // SAFETY: nginx supplied the configuration pointer while still
            // single-threaded.
            unsafe { postconfiguration_inner(cf) }
        })
    }
}

unsafe fn postconfiguration_inner(cf: *mut ngx_conf_t) -> ngx_int_t {
    // SAFETY: postconfiguration runs once in the single-threaded master before
    // workers fork; installing filters and the content handler is safe.
    unsafe {
        ngx_compress_ffi::filter::install(Some(header_filter), Some(body_filter));
        if crate::static_file::register(cf).is_err() {
            return Status::NGX_ERROR.0;
        }
    }
    Status::NGX_OK.0
}

// SAFETY: LocationConf is a plain POD config that ngx-rust allocates, default-
// initializes, and merges through the module's create/merge_loc_conf callbacks.
unsafe impl HttpModuleLocationConf for Module {
    type LocationConf = CompressConfig;
}

const fn directive(name: ngx_str_t) -> ngx_command_t {
    ngx_command_t {
        name,
        type_: (NGX_HTTP_MAIN_CONF | NGX_HTTP_SRV_CONF | NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1)
            as ngx_uint_t,
        set: Some(set_directive),
        conf: NGX_HTTP_LOC_CONF_OFFSET,
        offset: 0,
        post: ptr::null_mut(),
    }
}

const fn multi(name: ngx_str_t, extra: ngx_uint_t, setter: SetFn) -> ngx_command_t {
    ngx_command_t {
        name,
        type_: (NGX_HTTP_MAIN_CONF | NGX_HTTP_SRV_CONF | NGX_HTTP_LOC_CONF) as ngx_uint_t | extra,
        set: Some(setter),
        conf: NGX_HTTP_LOC_CONF_OFFSET,
        offset: 0,
        post: ptr::null_mut(),
    }
}

type SetFn = extern "C" fn(
    *mut ngx_conf_t,
    *mut ngx_command_t,
    *mut core::ffi::c_void,
) -> *mut core::ffi::c_char;

static mut NGX_HTTP_COMPRESS_COMMANDS: [ngx_command_t; 16] = [
    directive(ngx_string!("compress")),
    directive(ngx_string!("compress_static")),
    directive(ngx_string!("compress_gzip")),
    directive(ngx_string!("compress_gzip_comp_level")),
    directive(ngx_string!("compress_deflate")),
    directive(ngx_string!("compress_deflate_comp_level")),
    directive(ngx_string!("compress_brotli")),
    directive(ngx_string!("compress_brotli_comp_level")),
    directive(ngx_string!("compress_brotli_window")),
    directive(ngx_string!("compress_zstd")),
    directive(ngx_string!("compress_zstd_comp_level")),
    directive(ngx_string!("compress_min_length")),
    directive(ngx_string!("compress_vary")),
    multi(
        ngx_string!("compress_buffers"),
        NGX_CONF_TAKE2 as ngx_uint_t,
        set_buffers,
    ),
    multi(
        ngx_string!("compress_types"),
        NGX_CONF_1MORE as ngx_uint_t,
        set_types,
    ),
    ngx_command_t::empty(),
];

static NGX_HTTP_COMPRESS_MODULE_CTX: ngx_http_module_t = ngx_http_module_t {
    preconfiguration: Some(Module::preconfiguration),
    postconfiguration: Some(Module::postconfiguration),
    create_main_conf: None,
    init_main_conf: None,
    create_srv_conf: None,
    merge_srv_conf: None,
    create_loc_conf: Some(Module::create_loc_conf),
    merge_loc_conf: Some(Module::merge_loc_conf),
};

// The `ngx_modules` table is only needed for a standalone cdylib build; the
// NGINX buildsystem generates it and passes `--no-default-features`.
#[cfg(feature = "export-modules")]
ngx::ngx_modules!(ngx_http_compress_module);

#[used]
#[allow(non_upper_case_globals)]
#[cfg_attr(not(feature = "export-modules"), unsafe(no_mangle))]
pub static mut ngx_http_compress_module: ngx_module_t = ngx_module_t {
    ctx: ptr::addr_of!(NGX_HTTP_COMPRESS_MODULE_CTX)
        .cast_mut()
        .cast(),
    // SAFETY: taking the address of a 'static array element yields a valid pointer.
    commands: unsafe { ptr::addr_of_mut!(NGX_HTTP_COMPRESS_COMMANDS[0]) },
    type_: NGX_HTTP_MODULE as ngx_uint_t,
    ..ngx_module_t::default()
};
