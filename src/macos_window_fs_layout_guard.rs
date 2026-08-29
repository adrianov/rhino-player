// Break AppKit's `_syncToolbarPosition` ↔ `_updateTitlebarContainerViewFrameIfNecessary`
// recursion on macOS 26.x fullscreen exit (`setCustomTitlebarHeight:`). Include!'d from
// `macos_window_fs.rs`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Once, OnceLock};

use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::sel;

type SyncToolbarFn = unsafe extern "C-unwind" fn(*mut AnyObject, Sel);

static INSTALL: Once = Once::new();
static ORIG_SYNC: OnceLock<SyncToolbarFn> = OnceLock::new();
static SYNC_DEPTH: AtomicUsize = AtomicUsize::new(0);

/// Install once (enter + exit paths): re-entrant `_syncToolbarPosition` returns so AppKit
/// cannot stack-overflow while leaving native fullscreen.
pub(crate) fn ensure_titlebar_layout_guard() {
    INSTALL.call_once(install_sync_toolbar_guard);
}

fn install_sync_toolbar_guard() {
    let Some(cls) = AnyClass::get(c"NSThemeFrame") else {
        eprintln!("[rhino] macos-fs: NSThemeFrame missing — titlebar layout guard not installed");
        return;
    };
    let Some(method) = cls.instance_method(sel!(_syncToolbarPosition)) else {
        eprintln!(
            "[rhino] macos-fs: _syncToolbarPosition missing — titlebar layout guard not installed"
        );
        return;
    };
    let orig: SyncToolbarFn = unsafe { std::mem::transmute(method.implementation()) };
    let _ = ORIG_SYNC.set(orig);
    let guarded: Imp = unsafe { std::mem::transmute(guarded_sync_toolbar as SyncToolbarFn) };
    unsafe {
        method.set_implementation(guarded);
    }
    eprintln!("[rhino] macos-fs: _syncToolbarPosition reentrancy guard installed");
}

unsafe extern "C-unwind" fn guarded_sync_toolbar(this: *mut AnyObject, cmd: Sel) {
    // Healthy AppKit never re-enters (probe maxDepth=1). The fullscreen-exit bug nests
    // thousands deep; cut nested calls so the loop cannot grow.
    if SYNC_DEPTH.fetch_add(1, Ordering::Relaxed) > 0 {
        SYNC_DEPTH.fetch_sub(1, Ordering::Relaxed);
        return;
    }
    if let Some(&orig) = ORIG_SYNC.get() {
        unsafe { orig(this, cmd) };
    }
    SYNC_DEPTH.fetch_sub(1, Ordering::Relaxed);
}
