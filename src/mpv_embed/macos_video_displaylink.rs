//! CVDisplayLink driver: fires off-main-thread on every screen vsync and asks the
//! [`RhinoMpvGlLayer`] to render one frame **iff** mpv has produced new content.
//!
//! Why CVDisplayLink (deprecated as of macOS 14) and not the modern `NSView.displayLink`
//! API:
//!
//! * The replacement runs its callback on the main thread (mode
//!   `NSEventTrackingRunLoopMode` + others). That's exactly the path AppKit modal
//!   tracking blocks — we'd reproduce the menu / popover freeze the native render path is
//!   supposed to fix.
//! * `CVDisplayLink` runs on a dedicated kernel thread, completely independent of
//!   `CFRunLoop` modes. It's still supported on macOS 26 (it just emits a warning we
//!   mute with the module-level `#![allow(deprecated)]`).
//!
//! We coalesce frames with a single AtomicBool: mpv's update callback flips it on,
//! the displayLink callback consumes it under a CGL lock. No frames are produced when
//! mpv is idle, so the GPU stays asleep.

#![allow(deprecated)]

use std::os::raw::c_void;
use std::ptr::{self, NonNull};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use objc2::rc::Retained;
use objc2_core_video::{
    CVDisplayLink, CVDisplayLinkCreateWithActiveCGDisplays, CVReturn, CVTimeStamp,
};

use super::macos_video_layer::RhinoMpvGlLayer;

/// State shared between mpv's update callback and the displayLink callback.
pub struct DisplayLinkDriver {
    /// `Some` until [`stop`] runs; held to keep the link alive.
    link: Option<Retained<CVDisplayLink>>,
    /// Heap-stable so the raw pointer we pass to CV stays valid even if
    /// `DisplayLinkDriver` itself moves.
    state: Box<DriverState>,
}

pub struct DriverState {
    layer: Retained<RhinoMpvGlLayer>,
    pending: AtomicBool,
}

impl DriverState {
    fn new(layer: Retained<RhinoMpvGlLayer>) -> Box<Self> {
        Box::new(Self {
            layer,
            pending: AtomicBool::new(false),
        })
    }

    /// Set by mpv's update callback (any thread).
    pub fn mark_pending(&self) {
        self.pending.store(true, Ordering::Release);
    }
}

impl DisplayLinkDriver {
    /// Create + start a CVDisplayLink wired to `layer`. Returns the running driver and a
    /// cheap handle suitable for handing to mpv's update callback.
    pub fn install(layer: Retained<RhinoMpvGlLayer>) -> Result<(Self, Arc<DriverStateHandle>), String> {
        let state = DriverState::new(layer);
        let mut link_ptr: *mut CVDisplayLink = ptr::null_mut();
        let err = unsafe {
            CVDisplayLinkCreateWithActiveCGDisplays(NonNull::from(&mut link_ptr))
        };
        if err != 0 || link_ptr.is_null() {
            return Err(format!("CVDisplayLinkCreateWithActiveCGDisplays failed: {err}"));
        }
        let link: Retained<CVDisplayLink> = unsafe {
            Retained::from_raw(link_ptr).ok_or("displayLink retain failed")?
        };
        let user_info = state.as_ref() as *const DriverState as *mut c_void;
        let err = unsafe { link.set_output_callback(Some(display_link_callback), user_info) };
        if err != 0 {
            return Err(format!("set_output_callback failed: {err}"));
        }
        let err = link.start();
        if err != 0 {
            return Err(format!("CVDisplayLinkStart failed: {err}"));
        }
        let handle = Arc::new(DriverStateHandle {
            ptr: state.as_ref() as *const DriverState,
        });
        Ok((Self { link: Some(link), state }, handle))
    }
}

impl Drop for DisplayLinkDriver {
    fn drop(&mut self) {
        if let Some(link) = self.link.take() {
            let _ = link.stop();
            // Detach our callback so any in-flight tick from CV becomes a no-op.
            unsafe {
                let _ = link.set_output_callback(None, ptr::null_mut());
            }
        }
        let _ = &self.state;
    }
}

/// Cheap, `Send + Sync` handle for the displayLink driver state. Used by mpv's update
/// callback (which must be `Send`). Safe to clone.
pub struct DriverStateHandle {
    ptr: *const DriverState,
}

unsafe impl Send for DriverStateHandle {}
unsafe impl Sync for DriverStateHandle {}

impl DriverStateHandle {
    pub fn mark_pending(&self) {
        if self.ptr.is_null() {
            return;
        }
        unsafe { (*self.ptr).mark_pending(); }
    }
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
    if state.pending.swap(false, Ordering::AcqRel) {
        state.layer.display_now();
    }
    0
}

