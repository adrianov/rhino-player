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

mod output_callback;

use self::output_callback::{create_display_link, start_display_link};
use std::ptr::{self};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use objc2::rc::Retained;
use objc2_core_video::CVDisplayLink;

use super::macos_video_layer::RhinoMpvGlLayer;

/// State shared between mpv's update callback and the displayLink callback.
pub struct DisplayLinkDriver {
    /// `Mutex` so main thread can **`CVDisplayLinkStop`** during **`vf clr`** without dropping the link.
    link: Mutex<Option<Retained<CVDisplayLink>>>,
    /// Heap-stable so the raw pointer we pass to CV stays valid even if
    /// `DisplayLinkDriver` itself moves.
    state: Box<DriverState>,
}

pub struct DriverState {
    layer: Retained<RhinoMpvGlLayer>,
    pending: AtomicBool,
    /// Main clears vapoursynth **`vf`** — suppress **`display_now`** / draw callbacks cross-thread.
    vf_teardown_suppress: AtomicBool,
}

impl DriverState {
    fn new(layer: Retained<RhinoMpvGlLayer>) -> Box<Self> {
        Box::new(Self {
            layer,
            pending: AtomicBool::new(false),
            vf_teardown_suppress: AtomicBool::new(false),
        })
    }

    /// Set by mpv's update callback (any thread). Ignored during **`vf`** teardown on main.
    pub fn mark_pending(&self) {
        if self.vf_teardown_suppress.load(Ordering::Acquire) {
            return;
        }
        self.pending.store(true, Ordering::Release);
    }
}

impl DisplayLinkDriver {
    /// Create + start a CVDisplayLink wired to `layer`. Returns the running driver and a
    /// cheap handle suitable for handing to mpv's update callback.
    pub fn install(
        layer: Retained<RhinoMpvGlLayer>,
    ) -> Result<(Self, Arc<DriverStateHandle>), String> {
        let state = DriverState::new(layer);
        let link = create_display_link()?;
        let user_info = state.as_ref() as *const DriverState as *mut c_void;
        start_display_link(&link, user_info)?;
        let handle = Arc::new(DriverStateHandle {
            ptr: state.as_ref() as *const DriverState,
        });
        Ok((
            Self {
                link: Mutex::new(Some(link)),
                state,
            },
            handle,
        ))
    }

    /// **`running=false`**: **`CVDisplayLinkStop`** — must **not** run inside **`display_link_callback`**.
    pub(crate) fn set_cv_running(&self, running: bool) -> Result<(), String> {
        let guard = self.link.lock().unwrap_or_else(|p| p.into_inner());
        let Some(ref link) = *guard else {
            return Err("CVDisplayLink already released".into());
        };
        let code = if running { link.start() } else { link.stop() };
        if code != 0 {
            return Err(format!(
                "CVDisplayLink {} failed: {code}",
                if running { "Start" } else { "Stop" }
            ));
        }
        Ok(())
    }
}

impl Drop for DisplayLinkDriver {
    fn drop(&mut self) {
        let Ok(mut guard) = self.link.lock() else {
            return;
        };
        if let Some(link) = guard.take() {
            let _ = link.stop();
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
        unsafe {
            (*self.ptr).mark_pending();
        }
    }

    /// True while **`begin_vf_teardown`** / **`end_vf_teardown`** bracket **`vf clr`** on main.
    pub fn vf_teardown_suppress_active(&self) -> bool {
        if self.ptr.is_null() {
            return false;
        }
        unsafe { (*self.ptr).vf_teardown_suppress.load(Ordering::Acquire) }
    }

    /// Serialize **`mpv`** render/display-link bumps vs **`vf clr`** (Smooth **off** mid-play).
    pub fn begin_vf_teardown(&self) {
        if self.ptr.is_null() {
            return;
        }
        unsafe {
            let s = &*self.ptr;
            s.vf_teardown_suppress.store(true, Ordering::Release);
            s.pending.store(false, Ordering::Release);
        }
    }

    pub fn end_vf_teardown(&self) {
        if self.ptr.is_null() {
            return;
        }
        unsafe {
            (*self.ptr)
                .vf_teardown_suppress
                .store(false, Ordering::Release);
        }
    }
}
