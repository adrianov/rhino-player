//! Parse mpv `chapter-list` ([`libmpv2_sys::mpv_format_MPV_FORMAT_NODE`]) into `(time_sec, title)` pairs.

use libmpv2::Mpv;
use std::ffi::{CStr, CString};

pub fn mpv_chapter_list(mpv: &Mpv) -> Vec<(f64, String)> {
    let mut out = Vec::new();
    let Ok(name) = CString::new("chapter-list") else {
        return out;
    };
    let mut root = std::mem::MaybeUninit::<libmpv2_sys::mpv_node>::uninit();
    let err = unsafe {
        libmpv2_sys::mpv_get_property(
            mpv.ctx.as_ptr(),
            name.as_ptr(),
            libmpv2_sys::mpv_format_MPV_FORMAT_NODE,
            root.as_mut_ptr().cast(),
        )
    };
    if err < 0 {
        return out;
    }
    let mut root = unsafe { root.assume_init() };
    unsafe {
        parse_root(&mut root, &mut out);
        libmpv2_sys::mpv_free_node_contents(&mut root);
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    out
}

unsafe fn parse_root(root: &mut libmpv2_sys::mpv_node, out: &mut Vec<(f64, String)>) {
    if root.format != libmpv2_sys::mpv_format_MPV_FORMAT_NODE_ARRAY {
        return;
    }
    let list_ptr = root.u.list;
    if list_ptr.is_null() {
        return;
    }
    let n = (*list_ptr).num as usize;
    let values = (*list_ptr).values;
    if values.is_null() || n == 0 {
        return;
    }
    for i in 0..n {
        let entry = values.add(i).read();
        if let Some((t, tit)) = parse_chapter_map(&entry) {
            let title = if tit.is_empty() {
                format!("Chapter {}", i.saturating_add(1))
            } else {
                tit
            };
            out.push((t, title));
        }
    }
}

unsafe fn parse_chapter_map(entry: &libmpv2_sys::mpv_node) -> Option<(f64, String)> {
    if entry.format != libmpv2_sys::mpv_format_MPV_FORMAT_NODE_MAP {
        return None;
    }
    let list_ptr = entry.u.list;
    if list_ptr.is_null() {
        return None;
    }
    let n = (*list_ptr).num as usize;
    let keys = (*list_ptr).keys;
    let vals = (*list_ptr).values;
    if keys.is_null() || vals.is_null() || n == 0 {
        return None;
    }
    let mut time = None::<f64>;
    let mut title = String::new();
    for i in 0..n {
        let key_ptr = *keys.add(i);
        if key_ptr.is_null() {
            continue;
        }
        let key = CStr::from_ptr(key_ptr).to_bytes();
        let vn = vals.add(i).read();
        if key == b"time" && vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_DOUBLE {
            time = Some(vn.u.double_);
        } else if key == b"title" && vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_STRING {
            let sp = vn.u.string;
            if !sp.is_null() {
                title = CStr::from_ptr(sp).to_string_lossy().into_owned();
            }
        }
    }
    Some((time?, title))
}
