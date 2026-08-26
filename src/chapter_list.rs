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

/// `time` / `title` fields harvested from one chapter map node.
#[derive(Default)]
struct ChapterFields {
    time: Option<f64>,
    title: String,
}

impl ChapterFields {
    /// Absorb one key/value pair; other keys and unexpected formats are ignored.
    fn absorb(&mut self, key: &[u8], vn: &libmpv2_sys::mpv_node) {
        if key == b"time" && vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_DOUBLE {
            self.time = Some(unsafe { vn.u.double_ });
        } else if key == b"title" && vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_STRING {
            // SAFETY: `vn.u.string` points at an NUL-terminated mpv-allocated string.
            let sp = unsafe { vn.u.string };
            if !sp.is_null() {
                self.title = unsafe { CStr::from_ptr(sp).to_string_lossy().into_owned() };
            }
        }
    }
}

unsafe fn parse_chapter_map(entry: &libmpv2_sys::mpv_node) -> Option<(f64, String)> {
    let (keys, vals) = map_keys_values(entry)?;
    let mut fields = ChapterFields::default();
    for (key_ptr, vn) in keys.iter().zip(vals.iter()) {
        if !key_ptr.is_null() {
            let key = unsafe { CStr::from_ptr(*key_ptr) }.to_bytes();
            fields.absorb(key, vn);
        }
    }
    Some((fields.time?, fields.title))
}

/// Keys and values of a non-empty `MPV_FORMAT_NODE_MAP`, or `None`.
///
/// # Safety
/// `entry.u.list` must point at a valid mpv node list, as guaranteed by mpv itself.
unsafe fn map_keys_values(
    entry: &libmpv2_sys::mpv_node,
) -> Option<(&[*const std::os::raw::c_char], &[libmpv2_sys::mpv_node])> {
    if entry.format != libmpv2_sys::mpv_format_MPV_FORMAT_NODE_MAP {
        return None;
    }
    let list_ptr = entry.u.list;
    if list_ptr.is_null() {
        return None;
    }
    let n = unsafe { (*list_ptr).num } as usize;
    let (keys, vals) = unsafe { ((*list_ptr).keys, (*list_ptr).values) };
    if keys.is_null() || vals.is_null() || n == 0 {
        return None;
    }
    // SAFETY: both pointers and their shared length `n` come from the mpv node list.
    Some((
        unsafe { std::slice::from_raw_parts(keys.cast(), n) },
        unsafe { std::slice::from_raw_parts(vals, n) },
    ))
}
