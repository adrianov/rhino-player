// Frame re-assert machinery for the legacy-fullscreen state owner (`macos_legacy_fs`), pulled
// in with `include!` — one flat module scope, split for size. Async AppKit zoom ops land after
// any in-code `setFrame:display:`, so targets set around enter/exit/native interleave are
// verified after a settle interval and re-applied until they stick (bounded).

use objc2::rc::Retained;

/// Convergence attempts when the frame does not match the target (each re-applies it).
const REASSERT_TRIES: u8 = 3;
/// Tick budget spent waiting out a foreign transition (native enter/exit, GDK fullscreen
/// transition flag) before giving the convergence up — the native layer legitimately owns the
/// frame meanwhile, and a new chain is armed when its exit starts. The give-up is logged.
const BUSY_DEFERS: u8 = 12;

thread_local! {
    /// Set by the AppKit live-resize observer since the last [`arm_user_resize_watch`]: a
    /// user-driven interactive resize landed while a windowed re-assert chain is pending, so
    /// the user owns the frame now and the chain stands down.
    static USER_LIVE_RESIZE_SEEN: Cell<bool> = const { Cell::new(false) };
}

/// Observe the AppKit live-resize pair for user-driven interactive resizes only: programmatic
/// frame writes (our own restore and re-apply `setFrame` calls, the aspect snap, GTK's stale
/// content-size adoption) never post the live-resize pair, so this event source cannot
/// misfire on the machinery it guards against — the blind spot of both dimension heuristics
/// and AppKit `windowDidMove`, which fires for every frame change. The observer is installed
/// once for the process and scoped to the main window; [`USER_LIVE_RESIZE_SEEN`] is reset
/// each time a windowed restore arms the watch.
fn arm_user_resize_watch(nswin: &NSWindow) {
    USER_LIVE_RESIZE_SEEN.with(|f| f.set(false));
    thread_local! {
        static INSTALLED: Cell<bool> = const { Cell::new(false) };
    }
    if INSTALLED.with(Cell::get) {
        return;
    }
    use block2::RcBlock;
    use objc2_app_kit::NSWindowWillStartLiveResizeNotification;
    use objc2_foundation::NSNotificationCenter;
    INSTALLED.set(true);
    let block = RcBlock::new(move |_notif| {
        USER_LIVE_RESIZE_SEEN.with(|f| f.set(true));
    });
    let center = NSNotificationCenter::defaultCenter();
    let _observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWindowWillStartLiveResizeNotification),
            Some(nswin),
            None,
            &block,
        )
    };
    // Drop both at scope end: the center retains its own copy of the block and stays
    // registered after the token is released (same trade-off as the occlusion/screen
    // observers in this repo).
    drop(_observer);
}

/// Whether a user-driven live resize landed since the windowed restore armed the watch.
fn user_live_resize_seen() -> bool {
    USER_LIVE_RESIZE_SEEN.with(Cell::get)
}
/// One-shot verification for frames set while async AppKit zoom ops are in flight: after a
/// settle interval, re-`setFrame` the target until it sticks (bounded). `target` is in AppKit
/// screen coords (`[x, y, w, h]`, `nswin.frame()` parts). The callback is bound to the current
/// session generation: a legacy enter or exit in the meantime makes it stale and inert.
/// `reapply_presentation` marks the cover flavor: a native cycle may have dropped the auto-hide
/// presentation bits the cover depends on. Windowed restores re-assert the frame only — the
/// exact prior presentation word was already restored.
fn schedule_frame_reassert(
    win: &adw::ApplicationWindow,
    target: [f64; 4],
    retry: u8,
    defers: u8,
    reapply_presentation: bool,
) {
    let session = SESSION.get();
    let w = win.clone();
    let _ = glib::source::timeout_add_local_once(
        crate::fullscreen_timing::TRANSITION_SETTLE,
        move || reassert_step(w, target, retry, defers, reapply_presentation, session),
    );
}

/// Whether the frame already matches the target: the re-assert converges. Cover flavor: a
/// native cycle can drop menu-bar/Dock auto-hide while leaving the frame matching, so the
/// presentation bits ride along on the accepted convergence check too.
fn reassert_converged(
    mtm: MainThreadMarker,
    nswin: &NSWindow,
    target: [f64; 4],
    reapply_presentation: bool,
) -> bool {
    if frame_parts(nswin.frame()) != target {
        return false;
    }
    if reapply_presentation {
        reapply_legacy_presentation(mtm);
    }
    true
}

/// Whether deliberate user geometry owns the window now and the restore stands down: a
/// windowed restore (presentation untouched) whose current frame is a pure move (target size
/// at a different origin) or a shrink/mixed resize (an axis below the target). The exit's
/// frame restore races GTK's content-size adoption of the style-mask flip — the window can
/// sit at a stale frame at least as large as the target in both axes (the cover's content
/// rect) — and only that oversized residue still deserves the restore re-assert; growth
/// during the retry window is covered by the live-resize event source, not dimensions. The
/// covered window cannot be dragged or resized, so the cover flavor never stands down.
fn user_geometry_overrides(nswin: &NSWindow, target: [f64; 4], reapply_presentation: bool) -> bool {
    if reapply_presentation {
        return false;
    }
    let f = frame_parts(nswin.frame());
    if f[2] == target[2] && f[3] == target[3] && (f[0] != target[0] || f[1] != target[1]) {
        return true;
    }
    !(f[2] >= target[2] && f[3] >= target[3])
}

/// Busy handling for one re-assert tick: wait out the foreign transition without consuming a
/// convergence attempt, or log the abandonment when the deferral budget exhausts. Returns
/// `true` when the tick is fully handled (deferred or abandoned) and the caller returns.
fn reassert_busy_defer_or_abandon(
    win: &adw::ApplicationWindow,
    nswin: &NSWindow,
    target: [f64; 4],
    retry: u8,
    defers: u8,
    reapply_presentation: bool,
) -> bool {
    if !busy_for_reassert(win, nswin) {
        return false;
    }
    if defers < BUSY_DEFERS {
        schedule_frame_reassert(win, target, retry, defers + 1, reapply_presentation);
    } else {
        // Budget exhausted while a foreign transition still owns the frame: the chain
        // gives up. Always-on log — a cover or restore that never lands must not vanish
        // silently.
        eprintln!(
            "[rhino] legacy-fs: frame re-assert abandoned busy target={target:?} retry={retry} defers={defers} native_fs={} transition={}",
            crate::macos_window::ns_window_is_native_fullscreen(nswin),
            crate::macos_window::gdk_macos_in_fullscreen_transition(win),
        );
    }
    true
}

fn reassert_step(
    win: adw::ApplicationWindow,
    target: [f64; 4],
    retry: u8,
    defers: u8,
    reapply_presentation: bool,
    session: u32,
) {
    if SESSION.get() != session {
        return;
    }
    let Some((mtm, nswin)) = reassert_target_window(&win) else {
        return;
    };
    if reassert_busy_defer_or_abandon(&win, &nswin, target, retry, defers, reapply_presentation) {
        return;
    }
    if reassert_converged(mtm, &nswin, target, reapply_presentation) {
        return;
    }
    if (!reapply_presentation && user_live_resize_seen())
        || user_geometry_overrides(&nswin, target, reapply_presentation)
    {
        return;
    }
    // The target-reached check above runs before the budget: a final successful
    // reapplication must report convergence, not a give-up.
    if retry >= REASSERT_TRIES {
        eprintln!("[rhino] legacy-fs: frame re-assert gave up target={target:?}");
        return;
    }
    apply_reassert_target(mtm, &win, &nswin, target, reapply_presentation);
    schedule_frame_reassert(&win, target, retry + 1, 0, reapply_presentation);
}

/// Convergence step body: re-insert cover presentation bits when flagged, re-apply the target
/// frame, and let GDK settle it.
fn apply_reassert_target(
    mtm: MainThreadMarker,
    win: &adw::ApplicationWindow,
    nswin: &NSWindow,
    target: [f64; 4],
    reapply_presentation: bool,
) {
    if reapply_presentation {
        // The native cycle may have dropped the auto-hide presentation bits; without them
        // `constrainFrameRect` caps the cover at screen-minus-menu-bar and the set won't stick.
        reapply_legacy_presentation(mtm);
    }
    nswin.setFrame_display(parts_frame(target), true);
    settle_legacy_frame(win);
    log_window_chrome(nswin, "reassert");
}

/// Main-thread NSWindow for the re-assert pass, or nothing when the shell is tearing down.
fn reassert_target_window(
    win: &adw::ApplicationWindow,
) -> Option<(MainThreadMarker, Retained<NSWindow>)> {
    let mtm = MainThreadMarker::new()?;
    let nswin = crate::macos_window::nswindow_for_widget(win)?;
    Some((mtm, nswin))
}

/// Another owner owns the window frame right now: native fullscreen, or a native transition in
/// flight. Session staleness is handled by the generation check above.
fn busy_for_reassert(win: &adw::ApplicationWindow, nswin: &NSWindow) -> bool {
    crate::macos_window::ns_window_is_native_fullscreen(nswin)
        || crate::macos_window::gdk_macos_in_fullscreen_transition(win)
}

/// Re-insert the auto-hide presentation bits (menu bar + Dock) lost during a native cycle that
/// ran over the legacy session.
fn reapply_legacy_presentation(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    let mut opts = app.presentationOptions();
    opts.insert(
        NSApplicationPresentationOptions::AutoHideDock
            | NSApplicationPresentationOptions::AutoHideMenuBar,
    );
    app.setPresentationOptions(opts);
}

/// Called when a native fullscreen exit starts. If a legacy session is live, the native
/// zoom-out will land on AppKit's rewritten stored normal frame instead of the cover;
/// converge back on it once the native transition settles.
pub fn note_native_exit_started(win: &adw::ApplicationWindow) {
    if !ACTIVE.get() {
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(nswin) = crate::macos_window::nswindow_for_widget(win) else {
        return;
    };
    let Some(screen) = legacy_screen_frame(&nswin, mtm) else {
        return;
    };
    // The legacy cover is applied (window is borderless here), so the content-rect conversion
    // matches the cover the enter path applied — derive the convergence target the same way.
    schedule_frame_reassert(win, frame_parts(nswin.frameRectForContentRect(screen)), 0, 0, true);
}
