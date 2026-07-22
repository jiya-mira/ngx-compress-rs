//! Typed snapshots of configuration values owned by other NGINX modules.
//!
//! Discovery and pointer arithmetic stay in this boundary. Callers receive a
//! copyable descriptor and a Rust-owned boolean, never a borrowed private
//! module configuration structure.

use core::ffi::c_void;
use core::slice;

use ngx::ffi::{
    NGX_CONF_FLAG, NGX_HTTP_LOC_CONF_OFFSET, NGX_HTTP_MODULE, ngx_command_t, ngx_cycle_t,
    ngx_flag_t, ngx_http_request_t, ngx_uint_t,
};

/// Metadata needed to read one public `flag` directive from HTTP loc-conf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpLocFlag {
    module_index: usize,
    offset: usize,
}

/// Rust-owned state of NGINX's built-in `gzip` directive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuiltinGzipState {
    pub enabled: bool,
}

/// Finds the built-in gzip flag through NGINX module and command metadata.
///
/// # Safety
///
/// `cycle` must reference the live configuration cycle while modules and their
/// command tables remain initialized.
#[must_use]
// SAFETY: the caller supplies the live cycle described above.
pub unsafe fn find_builtin_gzip(cycle: *mut ngx_cycle_t) -> Option<HttpLocFlag> {
    if cycle.is_null() {
        return None;
    }

    // SAFETY: the cycle owns `modules_n` initialized module pointers.
    let modules = unsafe { slice::from_raw_parts((*cycle).modules, (*cycle).modules_n) };
    // Metadata traversal needs early continue/return. style:allow-for-in
    for module in modules.iter().copied() {
        if module.is_null() {
            continue;
        }
        // SAFETY: every pointer in the cycle module table references a live module.
        if unsafe { (*module).type_ } != NGX_HTTP_MODULE as _ {
            continue;
        }
        // SAFETY: a non-null NGINX command table ends with ngx_null_command.
        let Some(command) = (unsafe { find_command((*module).commands, b"gzip") }) else {
            continue;
        };
        // SAFETY: `command` points into the live command table.
        let command = unsafe { &*command };
        let is_loc_flag = command.conf == NGX_HTTP_LOC_CONF_OFFSET as ngx_uint_t
            && command.type_ & NGX_CONF_FLAG as ngx_uint_t != 0;
        if is_loc_flag {
            return Some(HttpLocFlag {
                // SAFETY: module is a live entry from the cycle table.
                module_index: unsafe { (*module).ctx_index },
                offset: command.offset,
            });
        }
    }
    None
}

/// Reads the built-in gzip state for a live request.
///
/// # Safety
///
/// `request` must reference a live NGINX HTTP request. `flag` must have been
/// discovered from the same configuration cycle.
#[must_use]
// SAFETY: the caller supplies a live request and same-cycle descriptor.
pub unsafe fn builtin_gzip_for_request(
    request: *mut ngx_http_request_t,
    flag: HttpLocFlag,
) -> Option<BuiltinGzipState> {
    if request.is_null() {
        return None;
    }
    // SAFETY: request loc_conf is a module-indexed array for its live cycle.
    unsafe { builtin_gzip_from_loc_conf((*request).loc_conf, flag) }
}

/// Reads the built-in gzip state from an effective HTTP loc-conf array.
///
/// # Safety
///
/// `loc_conf` must be a live NGINX HTTP location configuration array from the
/// same cycle used to discover `flag`.
#[must_use]
// SAFETY: the caller supplies a live same-cycle loc-conf array.
pub unsafe fn builtin_gzip_from_loc_conf(
    loc_conf: *mut *mut c_void,
    flag: HttpLocFlag,
) -> Option<BuiltinGzipState> {
    if loc_conf.is_null() {
        return None;
    }
    // SAFETY: descriptor module_index addresses this same-cycle loc-conf array.
    let module_conf = unsafe { *loc_conf.add(flag.module_index) };
    if module_conf.is_null() {
        return None;
    }
    // SAFETY: command metadata declares an ngx_flag_t at this byte offset.
    let value = unsafe {
        module_conf
            .cast::<u8>()
            .add(flag.offset)
            .cast::<ngx_flag_t>()
            .read_unaligned()
    };
    Some(BuiltinGzipState {
        enabled: value == 1,
    })
}

/// Gets a typed module configuration pointer from an HTTP loc-conf array.
///
/// # Safety
///
/// `loc_conf` must contain at least `module_index + 1` entries and the selected
/// entry must either be null or point to a live `T`.
#[must_use]
// SAFETY: the caller owns the array length and type invariant above.
pub unsafe fn location_conf<T>(loc_conf: *mut *mut c_void, module_index: usize) -> Option<*mut T> {
    if loc_conf.is_null() {
        return None;
    }
    // SAFETY: caller guarantees this index and type match the module table.
    let value = unsafe { *loc_conf.add(module_index) }.cast::<T>();
    (!value.is_null()).then_some(value)
}

// SAFETY: commands is a live NGINX table terminated by ngx_null_command.
unsafe fn find_command(
    mut commands: *mut ngx_command_t,
    name: &[u8],
) -> Option<*mut ngx_command_t> {
    while !commands.is_null() {
        // SAFETY: command tables terminate with a zero-length name.
        let command = unsafe { &*commands };
        if command.name.len == 0 {
            return None;
        }
        // SAFETY: NGINX command names are live byte strings for process lifetime.
        if unsafe { ngx_string_eq(&command.name, name) } {
            return Some(commands);
        }
        // SAFETY: command tables are contiguous through the terminator.
        commands = unsafe { commands.add(1) };
    }
    None
}

// SAFETY: value points to a live process-lifetime NGINX string.
unsafe fn ngx_string_eq(value: &ngx::ffi::ngx_str_t, expected: &[u8]) -> bool {
    value.len == expected.len()
        && !value.data.is_null()
        // SAFETY: NGINX owns `len` readable command-name bytes.
        && unsafe { slice::from_raw_parts(value.data, value.len) } == expected
}
