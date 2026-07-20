//! The `ngx_http_compress_module` definition: directive, configuration merge,
//! and the identity pass-through output filters (M1).

use core::ffi::{c_char, c_void};
use core::ptr;

use ngx::core::Status;
use ngx::ffi::{
    NGX_CONF_TAKE1, NGX_HTTP_LOC_CONF, NGX_HTTP_LOC_CONF_OFFSET, NGX_HTTP_MAIN_CONF,
    NGX_HTTP_MODULE, NGX_HTTP_SRV_CONF, NGX_LOG_EMERG, ngx_chain_t, ngx_command_t, ngx_conf_t,
    ngx_http_module_t, ngx_http_request_t, ngx_int_t, ngx_module_t, ngx_str_t, ngx_uint_t,
};
use ngx::http::{HttpModule, HttpModuleLocationConf, Merge, MergeConfigError, Request};
use ngx::{ngx_conf_log_error, ngx_string};
use ngx_compress_ffi::filter;

/// Location configuration. M1 exposes only the `compress on|off` master switch;
/// `None` means "inherit", so a child location can both enable and disable it.
#[derive(Debug, Default)]
struct CompressConfig {
    enable: Option<bool>,
}

impl CompressConfig {
    fn enabled(&self) -> bool {
        self.enable.unwrap_or(false)
    }
}

impl Merge for CompressConfig {
    fn merge(&mut self, prev: &Self) -> Result<(), MergeConfigError> {
        if self.enable.is_none() {
            self.enable = prev.enable;
        }
        Ok(())
    }
}

struct Module;

impl HttpModule for Module {
    fn module() -> &'static ngx_module_t {
        // SAFETY: the module static is initialized at load time and never moved.
        unsafe { &*ptr::addr_of!(ngx_http_compress_module) }
    }

    unsafe extern "C" fn postconfiguration(_cf: *mut ngx_conf_t) -> ngx_int_t {
        // SAFETY: postconfiguration runs once in the single-threaded master
        // before workers fork, so installing into the filter chains is safe.
        unsafe { filter::install(Some(header_filter), Some(body_filter)) };
        Status::NGX_OK.0
    }
}

// SAFETY: LocationConf is a plain POD config that ngx-rust allocates, default-
// initializes, and merges through the module's create/merge_loc_conf callbacks.
unsafe impl HttpModuleLocationConf for Module {
    type LocationConf = CompressConfig;
}

unsafe extern "C" fn header_filter(request: *mut ngx_http_request_t) -> ngx_int_t {
    // SAFETY: nginx invokes header filters with a valid request pointer.
    let req = unsafe { Request::from_ngx_http_request(request) };

    // When enabled, mark the response so integration tests can confirm the
    // module ran. A failed insertion means pool exhaustion, a real error.
    if Module::location_conf(req).is_some_and(CompressConfig::enabled)
        && req.add_header_out("X-Compress-Module", "active").is_none()
    {
        return Status::NGX_ERROR.0;
    }

    // SAFETY: install() ran during postconfiguration before any request.
    unsafe { filter::next_header(request) }
}

unsafe extern "C" fn body_filter(
    request: *mut ngx_http_request_t,
    chain: *mut ngx_chain_t,
) -> ngx_int_t {
    // Identity pass-through: forward the buffer chain unchanged (M1).
    // SAFETY: install() ran during postconfiguration before any request.
    unsafe { filter::next_body(request, chain) }
}

extern "C" fn set_enable(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    // SAFETY: nginx passes a valid cf and a pointer to our CompressConfig, and
    // NGX_CONF_TAKE1 guarantees exactly one argument in args[1].
    unsafe {
        let config = &mut *conf.cast::<CompressConfig>();
        let args: &[ngx_str_t] = (*(*cf).args).as_slice(); // style:allow-explicit-type
        match args[1].to_str() {
            Ok(value) if value.eq_ignore_ascii_case("on") => config.enable = Some(true),
            Ok(value) if value.eq_ignore_ascii_case("off") => config.enable = Some(false),
            Ok(_) => {
                ngx_conf_log_error!(
                    NGX_LOG_EMERG,
                    cf,
                    "invalid `compress` value; use `on` or `off`"
                );
                return ngx::core::NGX_CONF_ERROR;
            }
            Err(_) => {
                ngx_conf_log_error!(NGX_LOG_EMERG, cf, "`compress` value is not valid UTF-8");
                return ngx::core::NGX_CONF_ERROR;
            }
        }
    }
    ngx::core::NGX_CONF_OK
}

static mut NGX_HTTP_COMPRESS_COMMANDS: [ngx_command_t; 2] = [
    ngx_command_t {
        name: ngx_string!("compress"),
        // FFI flag composition widens the u32 config constants to ngx_uint_t.
        type_: (NGX_HTTP_MAIN_CONF | NGX_HTTP_SRV_CONF | NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1)
            as ngx_uint_t,
        set: Some(set_enable),
        conf: NGX_HTTP_LOC_CONF_OFFSET,
        offset: 0,
        post: ptr::null_mut(),
    },
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
    // SAFETY: taking the address of a 'static array element yields a valid, stable pointer.
    commands: unsafe { ptr::addr_of_mut!(NGX_HTTP_COMPRESS_COMMANDS[0]) },
    type_: NGX_HTTP_MODULE as ngx_uint_t,
    ..ngx_module_t::default()
};
