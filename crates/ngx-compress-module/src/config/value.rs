//! Typed parsing for already-owned NGINX directive values.

use ngx_compress_core::StaticMode;

pub(in crate::config) fn set_static(slot: &mut Option<StaticMode>, value: &str) -> bool {
    let mode = if value.eq_ignore_ascii_case("off") {
        StaticMode::Off
    } else if value.eq_ignore_ascii_case("on") {
        StaticMode::On
    } else if value.eq_ignore_ascii_case("always") {
        StaticMode::Always
    } else {
        return false;
    };
    *slot = Some(mode);
    true
}

pub(in crate::config) fn set_flag(slot: &mut Option<bool>, value: &str) -> bool {
    if value.eq_ignore_ascii_case("on") {
        *slot = Some(true);
        true
    } else if value.eq_ignore_ascii_case("off") {
        *slot = Some(false);
        true
    } else {
        false
    }
}

pub(in crate::config) fn set_u32(slot: &mut Option<u32>, value: &str, min: u32, max: u32) -> bool {
    match value.parse::<u32>() {
        Ok(parsed) if (min..=max).contains(&parsed) => {
            *slot = Some(parsed);
            true
        }
        _ => false,
    }
}

pub(in crate::config) fn set_zstd_level(slot: &mut Option<i32>, value: &str) -> bool {
    match value.parse::<i32>() {
        Ok(parsed) if (-7..=22).contains(&parsed) => {
            *slot = Some(parsed);
            true
        }
        _ => false,
    }
}

pub(in crate::config) fn set_usize(slot: &mut Option<usize>, value: &str) -> bool {
    match value.parse::<usize>() {
        Ok(parsed) => {
            *slot = Some(parsed);
            true
        }
        Err(_) => false,
    }
}
