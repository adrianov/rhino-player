use block2::RcBlock;
use glib::prelude::*;
use objc2_app_kit::{
    NSApplicationDidBecomeActiveNotification, NSApplicationDidResignActiveNotification,
    NSNormalWindowLevel,
};
use objc2_foundation::{NSNotificationCenter, NSNotificationName};

// Level swap + activation heal on app activation: while the legacy cover owns the window it
// floats above every ordinary window, so cmd+tab to another app cannot bring that app's
// windows forward — the player keeps covering the screen. Swap the level to normal while
// another app is active and back to floating on re-activation; the frame, style mask, and
// session state stay put, only z-order changes (IINA performs the same swap around app
// activation during its legacy fullscreen). Re-activation also drops the AppKit snapshot
// that stamps the stretched header band (see `invalidate_window_layers`). The observers are
// wired once per process; the handlers stand down when no fullscreen (legacy cover or
// GTK-native) is live, so windowed states are untouched; only the legacy branch swaps the
// window level.

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
    let Some(app) = app_weak.upgrade() else {
        return;
    };
    let Some(win) = crate::window_present::pick_application_window(&app) else {
        return;
    };
    // Legacy cover sessions and GTK-native fullscreen both share the AppKit snapshot
    // replay that stamps the band on activation; the level swap stays legacy-only
    // (native fullscreen owns its Space and must keep AppKit's z-order).
    let legacy = ACTIVE.get();
    if !legacy && !win.is_fullscreen() {
        return;
    }
    if legacy {
        let Some(nswin) = crate::macos_window::nswindow_for_widget(&win) else {
            return;
        };
        nswin.setLevel(if became_active {
            NSFloatingWindowLevel
        } else {
            NSNormalWindowLevel
        });
    }
    if became_active {
        // Drop the AppKit Space-switch snapshot the moment we are re-activated — it is
        // the stale, stretched chrome sublayer behind the mid-screen header band (see
        // `invalidate_window_layers`). The delayed repaint below re-heals if the replay
        // lands after this call.
        crate::macos_window::invalidate_window_layers(&win);
        if legacy {
            // Always-on: pairs the heal with the activation that drove it.
            eprintln!("[rhino] legacy-fs: activate — layers invalidated");
        }
        crate::macos_fs_debug::dump_window_layers(&win, "activate");
        schedule_activate_repaint(&win);
    }
    if legacy {
        // Always-on: a wrong z-order during legacy fullscreen is a user-visible regression
        // and the log line pairs the level flip with the activation event that drove it.
        // Native fullscreen never swaps the level, so this line stays legacy-only.
        eprintln!(
            "[rhino] legacy-fs: app {} — level {}",
            if became_active { "active" } else { "inactive" },
            if became_active { "floating" } else { "normal" },
        );
    }
}

/// Re-activation can replay the windowed geometry for a transient render (maximized-residue
/// latch interplay) and AppKit's Space-switch snapshot replay can land after the immediate
/// invalidate in [`on_app_active_changed`]: the chrome sublayer keeps stale, stretched
/// contents and the stretched strip stays baked mid-screen. After the settle, re-drop the
/// cached backing store and commit a full-window draw that rewrites every pixel.
fn schedule_activate_repaint(win: &adw::ApplicationWindow) {
    use gtk::prelude::WidgetExt;
    let w2 = win.clone();
    let _ = glib::timeout_add_local_once(
        crate::fullscreen_timing::TRANSITION_SETTLE * 2,
        move || {
            if !ACTIVE.get() && !w2.is_fullscreen() {
                return;
            }
            w2.queue_draw();
            crate::macos_window::invalidate_window_layers(&w2);
            // Always-on: pairs the heal with the activation that drove it.
            eprintln!("[rhino] legacy-fs: activate repaint queued");
            crate::macos_fs_debug::dump_window_layers(&w2, "activate_settled");
        },
    );
}
