// Raw `mpv_command_ret` + `mpv_node` map readers for `screenshot-raw` (no copies; caller frees the node).

fn mpv_command_ret(m: &Mpv, args: &[&str]) -> Option<libmpv2_sys::mpv_node> {
    let mut cstr_args: Vec<CString> = Vec::with_capacity(args.len());
    for arg in args {
        cstr_args.push(CString::new(*arg).ok()?);
    }
    let mut ptrs: Vec<_> = cstr_args.iter().map(|c| c.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    let mut result = std::mem::MaybeUninit::<libmpv2_sys::mpv_node>::zeroed();
    let err = unsafe {
        libmpv2_sys::mpv_command_ret(m.ctx.as_ptr(), ptrs.as_mut_ptr(), result.as_mut_ptr())
    };
    if err < 0 {
        eprintln!("[rhino] grid_thumb screenshot-raw mpv err={err}");
        return None;
    }
    Some(unsafe { result.assume_init() })
}

unsafe fn map_i64(map: &libmpv2_sys::mpv_node, want: &[u8]) -> Option<i64> {
    let vn = map_field(map, want)?;
    if vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_INT64 {
        Some(vn.u.int64)
    } else if vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_DOUBLE {
        Some(vn.u.double_ as i64)
    } else {
        None
    }
}

unsafe fn map_format_str<'a>(map: &'a libmpv2_sys::mpv_node, want: &[u8]) -> Option<&'a str> {
    let vn = map_field(map, want)?;
    if vn.format != libmpv2_sys::mpv_format_MPV_FORMAT_STRING {
        return None;
    }
    let sp = vn.u.string;
    if sp.is_null() {
        return None;
    }
    CStr::from_ptr(sp).to_str().ok()
}

unsafe fn map_byte_slice<'a>(map: &'a libmpv2_sys::mpv_node, want: &[u8]) -> Option<&'a [u8]> {
    let vn = map_field(map, want)?;
    if vn.format != libmpv2_sys::mpv_format_MPV_FORMAT_BYTE_ARRAY {
        return None;
    }
    let ba = vn.u.ba;
    if ba.is_null() {
        return None;
    }
    let data = (*ba).data;
    let size = (*ba).size;
    if data.is_null() || size == 0 {
        return None;
    }
    Some(std::slice::from_raw_parts(data.cast::<u8>(), size))
}

unsafe fn map_field<'a>(
    map: &'a libmpv2_sys::mpv_node,
    want: &[u8],
) -> Option<&'a libmpv2_sys::mpv_node> {
    if map.format != libmpv2_sys::mpv_format_MPV_FORMAT_NODE_MAP {
        return None;
    }
    let list_ptr = map.u.list;
    if list_ptr.is_null() {
        return None;
    }
    let n = (*list_ptr).num as usize;
    let keys = (*list_ptr).keys;
    let vals = (*list_ptr).values;
    if keys.is_null() || vals.is_null() || n == 0 {
        return None;
    }
    for i in 0..n {
        let key_ptr = *keys.add(i);
        if key_ptr.is_null() {
            continue;
        }
        if CStr::from_ptr(key_ptr).to_bytes() != want {
            continue;
        }
        return Some(&*vals.add(i));
    }
    None
}
