//! Discovery of the built-in gzip directive and configuration-time conflicts.

use core::ptr;
use std::collections::HashSet;

use ngx::ffi::{
    NGX_LOG_WARN, ngx_conf_t, ngx_http_core_loc_conf_t, ngx_http_core_main_conf_t,
    ngx_http_core_module, ngx_http_core_srv_conf_t, ngx_http_location_queue_t, ngx_queue_t,
};
use ngx::http::{HttpModule, HttpModuleMainConf};
use ngx::ngx_conf_log_error;
use ngx_compress_ffi::module_conf;

use crate::{BuiltinGzipRegistration, CompressConfig, Module, ResolveConfig, disabled_reason};

impl BuiltinGzipRegistration for Module {
    // SAFETY: caller must pass the live postconfiguration pointer.
    unsafe fn discover_gzip_and_warn(cf: *mut ngx_conf_t) -> Result<(), ()> {
        // SAFETY: forwards the trait's live postconfiguration contract.
        unsafe { discover_and_warn(cf) }
    }
}

/// Discovers gzip metadata, records expected conflicts, and emits `nginx -t`
/// warnings after NGINX has merged every server/location configuration.
// SAFETY: caller must pass the live postconfiguration pointer.
unsafe fn discover_and_warn(cf: *mut ngx_conf_t) -> Result<(), ()> {
    if cf.is_null() {
        return Err(());
    }
    // SAFETY: the live cycle module table is initialized before HTTP postconfig.
    let builtin_gzip = unsafe { module_conf::find_builtin_gzip((*cf).cycle) };
    // SAFETY: create_main_conf allocated MainConfig for this configuration.
    let main = unsafe { Module::main_conf_mut(&*cf) }.ok_or(())?;
    main.builtin_gzip = builtin_gzip;
    let Some(flag) = builtin_gzip else {
        return Ok(());
    };

    // SAFETY: cf.ctx is the live HTTP configuration context.
    let ctx = unsafe { (*cf).ctx.cast::<ngx::ffi::ngx_http_conf_ctx_t>().as_ref() }.ok_or(())?;
    // SAFETY: core module main conf is a live ngx_http_core_main_conf_t.
    let core_index = unsafe { (*ptr::addr_of!(ngx_http_core_module)).ctx_index };
    // SAFETY: main_conf is indexed by the initialized core module ctx_index.
    let cmcf = unsafe {
        (*ctx.main_conf.add(core_index))
            .cast::<ngx_http_core_main_conf_t>()
            .as_ref()
    }
    .ok_or(())?;
    let module_index = Module::module().ctx_index;
    let mut seen_locations = HashSet::new();
    let mut seen_configs = HashSet::new();

    // SAFETY: NGINX owns `nelts` server pointers in this array.
    let servers = unsafe {
        core::slice::from_raw_parts(
            cmcf.servers.elts.cast::<*mut ngx_http_core_srv_conf_t>(),
            cmcf.servers.nelts,
        )
    };
    // Pointer traversal with fallible early returns is clearer as a loop. style:allow-for-in
    for server in servers.iter().copied().filter(|server| !server.is_null()) {
        // SAFETY: every server entry owns a live HTTP configuration context.
        let server_ctx = unsafe { (*server).ctx.as_ref() }.ok_or(())?;
        // SAFETY: locate the server root core loc-conf by core ctx_index.
        let root =
            unsafe { (*server_ctx.loc_conf.add(core_index)).cast::<ngx_http_core_loc_conf_t>() };
        // SAFETY: traversal stays within NGINX-owned core location structures.
        unsafe {
            // The server root's effective module configs live directly on its
            // HTTP context; unlike explicit locations, core's root loc-conf
            // does not populate its own `loc_conf` back-pointer.
            inspect_loc_conf(
                cf,
                server_ctx.loc_conf,
                module_index,
                flag,
                &mut seen_configs,
            );
            scan_location_tree(
                cf,
                root,
                module_index,
                flag,
                &mut seen_locations,
                &mut seen_configs,
            )?;
            scan_null_terminated_locations(
                cf,
                (*server).named_locations,
                module_index,
                flag,
                &mut seen_locations,
                &mut seen_configs,
            )?;
        }
    }
    Ok(())
}

// SAFETY: all pointers belong to the current NGINX configuration tree.
unsafe fn scan_location_tree(
    cf: *mut ngx_conf_t,
    location: *mut ngx_http_core_loc_conf_t,
    module_index: usize,
    flag: module_conf::HttpLocFlag,
    seen_locations: &mut HashSet<usize>,
    seen_configs: &mut HashSet<usize>,
) -> Result<(), ()> {
    if location.is_null() || !seen_locations.insert(location.addr()) {
        return Ok(());
    }
    // SAFETY: location is live and its loc-conf array uses current module indices.
    unsafe {
        inspect_loc_conf(cf, (*location).loc_conf, module_index, flag, seen_configs);
        inspect_loc_conf(
            cf,
            (*location).limit_except_loc_conf,
            module_index,
            flag,
            seen_configs,
        );
        scan_location_queue(
            cf,
            (*location).locations,
            module_index,
            flag,
            seen_locations,
            seen_configs,
        )?;
        scan_null_terminated_locations(
            cf,
            (*location).regex_locations,
            module_index,
            flag,
            seen_locations,
            seen_configs,
        )?;
    }
    Ok(())
}

// SAFETY: `head` is a live circular NGINX location queue.
unsafe fn scan_location_queue(
    cf: *mut ngx_conf_t,
    head: *mut ngx_queue_t,
    module_index: usize,
    flag: module_conf::HttpLocFlag,
    seen_locations: &mut HashSet<usize>,
    seen_configs: &mut HashSet<usize>,
) -> Result<(), ()> {
    if head.is_null() {
        return Ok(());
    }
    // SAFETY: `head` is a circular NGINX queue sentinel.
    let mut node = unsafe { (*head).next };
    while !node.is_null() && node != head {
        // SAFETY: `queue` is the first field and has the same pointer alignment.
        let entry = node.cast::<ngx_http_location_queue_t>();
        // SAFETY: exact/inclusive point to live child core location confs.
        unsafe {
            scan_location_tree(
                cf,
                (*entry).exact,
                module_index,
                flag,
                seen_locations,
                seen_configs,
            )?;
            scan_location_tree(
                cf,
                (*entry).inclusive,
                module_index,
                flag,
                seen_locations,
                seen_configs,
            )?;
            node = (*node).next;
        }
    }
    Ok(())
}

// SAFETY: `locations` is a live null-terminated NGINX pointer array.
unsafe fn scan_null_terminated_locations(
    cf: *mut ngx_conf_t,
    mut locations: *mut *mut ngx_http_core_loc_conf_t,
    module_index: usize,
    flag: module_conf::HttpLocFlag,
    seen_locations: &mut HashSet<usize>,
    seen_configs: &mut HashSet<usize>,
) -> Result<(), ()> {
    if locations.is_null() {
        return Ok(());
    }
    // SAFETY: NGINX regex/named location arrays are null-terminated.
    while unsafe { !(*locations).is_null() } {
        // SAFETY: current entry is a live core loc-conf from this cycle.
        unsafe {
            scan_location_tree(
                cf,
                *locations,
                module_index,
                flag,
                seen_locations,
                seen_configs,
            )?;
            locations = locations.add(1);
        }
    }
    Ok(())
}

// SAFETY: loc_conf is a live array indexed by current-cycle module ctx indices.
unsafe fn inspect_loc_conf(
    cf: *mut ngx_conf_t,
    loc_conf: *mut *mut core::ffi::c_void,
    module_index: usize,
    flag: module_conf::HttpLocFlag,
    seen_configs: &mut HashSet<usize>,
) {
    // SAFETY: array is indexed by this module's current ctx_index.
    let Some(config) =
        (unsafe { module_conf::location_conf::<CompressConfig>(loc_conf, module_index) })
    else {
        return;
    };
    if !seen_configs.insert(config.addr()) {
        return;
    }
    // SAFETY: config is the exact type allocated by this module.
    let config = unsafe { &mut *config };
    // SAFETY: the descriptor and loc-conf are from the same live cycle.
    let gzip = unsafe { module_conf::builtin_gzip_from_loc_conf(loc_conf, flag) };
    let conflict = disabled_reason(config.resolve().enabled, gzip).is_some();
    config.gzip_conflict_expected = conflict;
    if conflict {
        ngx_conf_log_error!(
            NGX_LOG_WARN,
            cf,
            "module=ngx_compress callback=postconfiguration class=builtin_gzip_conflict: gzip on disables runtime compression and sidecar handling for this location"
        );
    }
}
