//! CVDisplayLink creation, callback registration, and the vsync output callback itself.
//! Split from `macos_video_displaylink.rs` to keep each module small.

#![allow(deprecated)]

use objc2::rc::Retained;
use objc2_core_video::{
    CVDisplayLink, CVDisplayLinkCreateWithActiveCGDisplays, CVReturn, CVTimeStamp,
};
use std::os::raw::c_void;
use std::ptr::{self, NonNull};
use std::sync::atomic::Ordering;

use super::DriverState;

/// Create a `CVDisplayLink` bound to the active displays.
pub(super) fn create_display_link() -> Result<Retained<CVDisplayLink>, String> {
    let mut link_ptr: *mut CVDisplayLink = ptr::null_mut();
    let err = unsafe { CVDisplayLinkCreateWithActiveCGDisplays(NonNull::from(&mut link_ptr)) };
    if err != 0 || link_ptr.is_null() {
        return Err(format!(
            "CVDisplayLinkCreateWithActiveCGDisplays failed: {err}"
        ));
    }
    let link: Retained<CVDisplayLink> =
        unsafe { Retained::from_raw(link_ptr).ok_or("displayLink retain failed")? };
    Ok(link)
}

/// Wire the output callback onto `link` and start it firing on every screen vsync.
pub(super) fn start_display_link(
    link: &CVDisplayLink,
    user_info: *mut c_void,
) -> Result<(), String> {
    let err = unsafe { link.set_output_callback(Some(display_link_callback), user_info) };
    if err != 0 {
        return Err(format!("set_output_callback failed: {err}"));
    }
    let err = link.start();
    if err != 0 {
        return Err(format!("CVDisplayLinkStart failed: {err}"));
    }
    Ok(())
}

/// CVDisplayLink output callback. Runs on the displayLink's dedicated kernel thread, so
/// it keeps firing even when the GTK/AppKit main thread is parked in a modal tracking
/// loop (menu / popover).
unsafe extern "C-unwind" fn display_link_callback(
    _link: ptr::NonNull<CVDisplayLink>,
    _now: ptr::NonNull<CVTimeStamp>,
    _output_time: ptr::NonNull<CVTimeStamp>,
    _flags_in: u64,
    _flags_out: ptr::NonNull<u64>,
    user_info: *mut c_void,
) -> CVReturn {
    if user_info.is_null() {
        return 0;
    }
    let state = unsafe { &*(user_info as *const DriverState) };
    if state.vf_teardown_suppress.load(Ordering::Acquire) {
        state.pending.store(false, Ordering::Release);
        return 0;
    }
    if state.pending.swap(false, Ordering::AcqRel) {
        state.layer.display_now();
    }
    0
}
