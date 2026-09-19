use block2::RcBlock;
use glib::prelude::*;
use objc2_app_kit::{
    NSApplicationDidBecomeActiveNotification, NSApplicationDidResignActiveNotification,
    NSNormalWindowLevel,
};
use objc2_foundation::{NSNotificationCenter, NSNotificationName};

// Legacy-fullscreen level swap on app activation: while the cover owns the window it floats
// above every ordinary window, so cmd+tab to another app cannot bring that app's windows
// forward — the player keeps covering the screen. Swap the level to normal while another app
// is active and back to floating on re-activation; the frame, style mask, and session state
// stay put, only z-order changes. (IINA performs the same level swap around app activation
// during its legacy fullscreen.) The observers are wired once per process; the handlers stand
// down when no legacy session is live, so native fullscreen and windowed states are untouched.

thread_local! {
    static LEVEL_SWAP_WIRED: Cell<bool> = const { Cell::new(false) };
}

fn wire_level_swap_on_app_active(win: &adw::ApplicationWindow) {
    if LEVEL_SWAP_WIRED.get() {
        return;
    }
    let Some(app) = win
        .application()
        .and_then(|a| a.downcast::<adw::Application>().ok())
    else {
        return;
    };
    let weak = app.downgrade();
    let center = NSNotificationCenter::defaultCenter();
    // NSApplicationDid{Resign,Become}ActiveNotification are extern statics: reading the
    // symbols is `unsafe` by objc2 contract; the values are AppKit-declared constants.
    observe_level_swap(
        &center,
        &weak,
        unsafe { NSApplicationDidResignActiveNotification },
        false,
    );
    observe_level_swap(
        &center,
        &weak,
        unsafe { NSApplicationDidBecomeActiveNotification },
        true,
    );
    LEVEL_SWAP_WIRED.set(true);
}

fn observe_level_swap(
    center: &NSNotificationCenter,
    app_weak: &glib::WeakRef<adw::Application>,
    name: &NSNotificationName,
    became_active: bool,
) {
    let weak = app_weak.clone();
    let block = RcBlock::new(move |_| {
        on_app_active_changed(&weak, became_active);
    });
    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
    };
    std::mem::forget(observer);
    LEVEL_SWAP_WIRED.set(true);
}

fn on_app_active_changed(app_weak: &glib::WeakRef<adw::Application>, became_active: bool) {
    if !ACTIVE.get() {
        return;
    }
    let Some(app) = app_weak.upgrade() else {
        return;
    };
    let Some(win) = crate::window_present::pick_application_window(&app) else {
        return;
    };
    let Some(nswin) = crate::macos_window::nswindow_for_widget(&win) else {
        return;
    };
    nswin.setLevel(if became_active {
        NSFloatingWindowLevel
    } else {
        NSNormalWindowLevel
    });
    if became_active {
        schedule_activate_repaint(&win);
    }
    // Always-on: a wrong z-order during legacy fullscreen is a user-visible regression and
    // the log line pairs the level flip with the activation event that drove it.
    eprintln!(
        "[rhino] legacy-fs: app {} — level {}",
        if became_active { "active" } else { "inactive" },
        if became_active { "floating" } else { "normal" },
    );
}

/// An activation can replay the windowed geometry for a transient render (maximized-residue
/// latch interplay): GSK commits a frame at the old windowed size, the layer stretches it to
/// the cover, and the stretched chrome strip stays baked mid-screen. The re-assert machinery
/// re-pins the frame but nothing repaints; a full-window draw after the settle rewrites every
/// pixel and heals the band.
fn schedule_activate_repaint(win: &adw::ApplicationWindow) {
    use gtk::prelude::WidgetExt;

    let w2 = win.clone();
    let _ = glib::timeout_add_local_once(
        crate::fullscreen_timing::TRANSITION_SETTLE * 2,
        move || {
            if !ACTIVE.get() {
                return;
            }
            w2.queue_draw();
            // Always-on: pairs the heal with the activation that drove it.
            eprintln!("[rhino] legacy-fs: activate repaint queued");
        },
    );
}
