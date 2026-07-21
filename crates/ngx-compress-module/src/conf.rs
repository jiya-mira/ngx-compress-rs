//! NGINX directive callbacks: prefetch external arguments, then call safe config.

use core::ffi::{c_char, c_void};

use ngx::core::{NGX_CONF_ERROR, NGX_CONF_OK};
use ngx::ffi::{NGX_LOG_EMERG, ngx_command_t, ngx_conf_t, ngx_parse_size, ngx_str_t};
use ngx::ngx_conf_log_error;

use crate::config::CompressConfig;

pub(crate) extern "C" fn set_directive(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    ngx_compress_ffi::guard::callback(NGX_CONF_ERROR, || {
        // SAFETY: nginx supplies valid configuration pointers to this setter.
        unsafe { set_directive_inner(cf, conf) }
    })
}

unsafe fn set_directive_inner(cf: *mut ngx_conf_t, conf: *mut c_void) -> *mut c_char {
    // SAFETY: TAKE1 guarantees name and value entries in the live args array.
    let values = unsafe {
        let args: &[ngx_str_t] = (*(*cf).args).as_slice(); // style:allow-explicit-type
        args.first()
            .and_then(|name| ngx_compress_ffi::string::copy_string(name))
            .zip(
                args.get(1)
                    .and_then(|value| ngx_compress_ffi::string::copy_string(value)),
            )
    };
    let Some((name, value)) = values else {
        ngx_conf_log_error!(NGX_LOG_EMERG, cf, "compress directive value is not UTF-8");
        return NGX_CONF_ERROR;
    };
    // SAFETY: nginx allocated and initialized this module configuration.
    let config = unsafe { &mut *conf.cast::<CompressConfig>() };
    if config.apply(&name, &value) {
        NGX_CONF_OK
    } else {
        ngx_conf_log_error!(NGX_LOG_EMERG, cf, "invalid value for compress directive");
        NGX_CONF_ERROR
    }
}

pub(crate) extern "C" fn set_buffers(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    ngx_compress_ffi::guard::callback(NGX_CONF_ERROR, || {
        // SAFETY: nginx supplies valid configuration pointers to this setter.
        unsafe { set_buffers_inner(cf, conf) }
    })
}

unsafe fn set_buffers_inner(cf: *mut ngx_conf_t, conf: *mut c_void) -> *mut c_char {
    // SAFETY: TAKE2 guarantees count and size entries in the live args array.
    let parsed = unsafe {
        let args: &[ngx_str_t] = (*(*cf).args).as_slice(); // style:allow-explicit-type
        let Some((count_arg, size)) = args.get(1).zip(args.get(2)) else {
            return NGX_CONF_ERROR;
        };
        let count = ngx_compress_ffi::string::copy_string(count_arg)
            .and_then(|count| count.parse::<usize>().ok());
        let mut size_arg = *size;
        let size = ngx_parse_size(&raw mut size_arg);
        count.zip(usize::try_from(size).ok())
    };
    if let Some((count, size)) = parsed.filter(|(count, size)| *count > 0 && *size > 0) {
        // SAFETY: nginx allocated and initialized this module configuration.
        let config = unsafe { &mut *conf.cast::<CompressConfig>() };
        config.set_buffers(count, size);
        NGX_CONF_OK
    } else {
        ngx_conf_log_error!(NGX_LOG_EMERG, cf, "invalid compress_buffers value");
        NGX_CONF_ERROR
    }
}

pub(crate) extern "C" fn set_types(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    ngx_compress_ffi::guard::callback(NGX_CONF_ERROR, || {
        // SAFETY: nginx supplies valid configuration pointers to this setter.
        unsafe { set_types_inner(cf, conf) }
    })
}

unsafe fn set_types_inner(cf: *mut ngx_conf_t, conf: *mut c_void) -> *mut c_char {
    // SAFETY: 1MORE guarantees at least one MIME entry in the live args array;
    // copy every value before leaving the FFI prefetch scope.
    let values = unsafe {
        let args: &[ngx_str_t] = (*(*cf).args).as_slice(); // style:allow-explicit-type
        args.get(1..)
            .filter(|values| !values.is_empty())
            .map(|values| {
                values
                    .iter()
                    .map(|mime| ngx_compress_ffi::string::copy_string(mime).ok_or(()))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
    };
    if let Ok(Some(values)) = values {
        // SAFETY: nginx allocated and initialized this module configuration.
        let config = unsafe { &mut *conf.cast::<CompressConfig>() };
        config.set_types(values);
        NGX_CONF_OK
    } else {
        ngx_conf_log_error!(NGX_LOG_EMERG, cf, "compress_types value is not UTF-8");
        NGX_CONF_ERROR
    }
}
