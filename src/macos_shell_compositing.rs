//! One gdk-macos hybrid-layer refresh policy for `outer_ovl` children.
//!
//! Video lives in a native `CAOpenGLLayer` under gdk-macos's GTK sublayer. Showing or
//! hiding an overlay child (header menu panel, seek preview) can leave stale chrome
//! tiles — especially when the window is fullscreen and not key. Callers only report
//! open/close; timing and invalidate live here.

use std::cell::{Cell, RefCell};

const HOLD_MS: u32 = 300;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static SETTLE: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}

pub fn armed() -> bool {
    ARMED.with(Cell::get)
}

fn cancel_settle() {
    SETTLE.with(crate::glib_source_drop::drop_glib_source);
}

fn set_armed(armed: bool) {
    ARMED.with(|a| a.set(armed));
}

fn settle_after(delay: std::time::Duration, callback: impl FnOnce() + 'static) {
    cancel_settle();
    let id = glib::timeout_add_local_once(delay, move || {
        SETTLE.with(crate::glib_source_drop::finish_glib_source);
        callback();
    });
    SETTLE.with(|slot| *slot.borrow_mut() = Some(id));
}

pub fn arm_hold() {
    set_armed(true);
    settle_after(std::time::Duration::from_millis(u64::from(HOLD_MS)), || {
        set_armed(false)
    });
}

pub fn disarm_hold() {
    cancel_settle();
    set_armed(false);
}

fn refresh() {
    crate::app::refresh_registered_shell_compositing();
}

fn settle_open() {
    settle_after(
        std::time::Duration::from_millis(u64::from(HOLD_MS) + 32),
        || {
            set_armed(false);
            refresh();
        },
    );
}

/// Header menu overlay became visible in theater.
/// Defers layer invalidate briefly to avoid a full-window flash, then refreshes again.
pub fn overlay_opened() {
    set_armed(true);
    refresh();
    settle_open();
}

/// After an `outer_ovl` child show/hide that can leave stale tiles: wait one tick for
/// GTK to commit the tree, refresh, then refresh again (one pass can replay a stale
/// snapshot when the window is not key). Cancels any pending arm/settle first.
fn flush_overlay_change() {
    disarm_hold();
    settle_after(std::time::Duration::ZERO, || {
        refresh();
        settle_after(
            std::time::Duration::from_millis(u64::from(HOLD_MS) + 32),
            refresh,
        );
    });
}

/// Seek preview became visible in theater.
///
/// Do **not** use [`overlay_opened`]'s deferred-arm path — that hold leaves titlebar
/// ghosts on the video (worst when fullscreen and not key). Same flush as close.
pub fn preview_opened() {
    flush_overlay_change();
}

/// Overlay child hidden (menu panel or seek preview).
pub fn overlay_closed() {
    flush_overlay_change();
}
