//! macOS fullscreen-exit guard: coalesce duplicate exit requests and block traffic-light
//! hides while AppKit leaves fullscreen (toolbar / zoom-cell updates during transition can crash).

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[cfg(target_os = "macos")]
static EXIT_ARMED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
static UNFS_GEN: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "macos")]
pub(crate) fn exit_armed() -> bool {
    EXIT_ARMED.load(Ordering::Acquire)
}

/// Arm the exit guard and bump the unfullscreen generation. Returns `None` when already armed.
#[cfg(target_os = "macos")]
pub(crate) fn try_arm_exit() -> Option<u64> {
    if EXIT_ARMED.swap(true, Ordering::AcqRel) {
        crate::macos_fs_debug::log("exit already armed (skip)");
        return None;
    }
    let gen = UNFS_GEN.fetch_add(1, Ordering::AcqRel) + 1;
    crate::macos_fs_debug::log("exit armed");
    Some(gen)
}

#[cfg(target_os = "macos")]
pub(crate) fn clear_exit() {
    if EXIT_ARMED.swap(false, Ordering::AcqRel) {
        crate::macos_fs_debug::log("exit cleared");
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn unfs_gen_is_current(gen: u64) -> bool {
    gen == UNFS_GEN.load(Ordering::Acquire)
}
