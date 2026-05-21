//! macOS fullscreen-exit guard: coalesce duplicate exit requests and block traffic-light
//! hides while AppKit leaves fullscreen (toolbar / zoom-cell updates during transition can crash).

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
static EXIT_ARMED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
pub(crate) fn exit_armed() -> bool {
    EXIT_ARMED.load(Ordering::Acquire)
}

/// Arm the exit guard. Returns false when an exit is already in flight.
#[cfg(target_os = "macos")]
pub(crate) fn try_arm_exit() -> bool {
    if EXIT_ARMED.swap(true, Ordering::AcqRel) {
        crate::macos_fs_debug::log("exit already armed (skip)");
        return false;
    }
    crate::macos_fs_debug::log("exit armed");
    true
}

#[cfg(target_os = "macos")]
pub(crate) fn clear_exit() {
    if EXIT_ARMED.swap(false, Ordering::AcqRel) {
        crate::macos_fs_debug::log("exit cleared");
    }
}
