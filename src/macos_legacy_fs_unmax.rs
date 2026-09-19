// Asynchronous unmaximize for the legacy-fullscreen state owner (`macos_legacy_fs`), pulled in
// with `include!` — one flat module scope, split for size. gdk-macos raises the *maximized*
// notify and drops the GTK flag when the unmaximize is requested, not when AppKit's async
// zoom-out lands, so neither the notify nor the GTK size proves the zoom has landed. The settle
// gate samples the AppKit frame itself: stable across ticks, not maximized, and no longer
// workarea-sized. Bound to the transition generation so a superseding request or native
// transition kills the wait.

thread_local! {
    /// Set from an unmaximize request until the frame has settled: later enter requests must go
    /// through the stability wait even though `is_maximized()` already reads false.
    static UNMAX_IN_FLIGHT: Cell<bool> = const { Cell::new(false) };
}

/// Whether an unmaximize request has not settled its zoom-out yet.
fn unmaximize_in_flight() -> bool {
    UNMAX_IN_FLIGHT.get()
}

/// Asynchronous unmaximize: cover only once the zoom-out has completed and the frame is stable.
/// Only the first request in flight issues `unmaximize()` — a re-request while AppKit's
/// zoom-out is still animating toggles the zoom back on, so a double-press just refreshes the
/// wait.
fn enter_after_unmaximize(win: &adw::ApplicationWindow) {
    let session = SESSION.get();
    let first_request = !UNMAX_IN_FLIGHT.replace(true);
    if first_request {
        win.unmaximize();
    }
    wait_unmaximized_settle(win.clone(), Rc::new(Cell::new(None)), 0, session);
}

/// ~100 ms ticks; enter once the AppKit frame has settled (~1.5 s budget). Every tick re-checks
/// validity before rescheduling or entering, and the budget terminates with a give-up instead of
/// restarting the wait.
fn wait_unmaximized_settle(
    win: adw::ApplicationWindow,
    last: Rc<Cell<Option<[f64; 4]>>>,
    tick: u8,
    session: u32,
) {
    const SETTLE_TICKS: u8 = 15;
    if !pending_entry_still_valid(&win, session) {
        // The owning request was cancelled (superseded, or a native transition took over the
        // window). Clear the latch so a later request still issues its own `unmaximize()` — but
        // never clear a newer session's request, which set the latch itself.
        if SESSION.get() == session {
            UNMAX_IN_FLIGHT.set(false);
        }
        return;
    }
    if unmaximize_settled(&win, &last, tick) {
        UNMAX_IN_FLIGHT.set(false);
        enter(&win);
        return;
    }
    if tick >= SETTLE_TICKS {
        // Budget spent and the final sample — a real ~100 ms gap — still failed the settle
        // checks: give up on that verdict. Re-sampling synchronously cannot detect a moving
        // frame (no time elapsed between samples just compares the frame against itself) and
        // would enter mid-zoom with an unsettled prior-frame snapshot. The next F press
        // retries cleanly.
        if SESSION.get() == session {
            UNMAX_IN_FLIGHT.set(false);
        }
        eprintln!("[rhino] legacy-fs: enter: unmaximize did not settle; giving up");
        return;
    }
    reschedule_unmaximized_wait(win, last, tick, session);
}

/// Next ~100 ms stability tick; this tick's AppKit frame sample becomes the comparison baseline.
fn reschedule_unmaximized_wait(
    win: adw::ApplicationWindow,
    last: Rc<Cell<Option<[f64; 4]>>>,
    tick: u8,
    session: u32,
) {
    let w2 = win.clone();
    let l2 = Rc::clone(&last);
    let _ = glib::source::timeout_add_local_once(
        std::time::Duration::from_millis(100),
        move || wait_unmaximized_settle(w2, l2, tick + 1, session),
    );
}

/// Whether the zoom-out has landed: AppKit frame (the ground truth the prior frame is captured
/// from) stable across ticks, GTK no longer reporting maximized, and the frame no longer
/// workarea-sized — a still-pending zoom-out sits at the maximized frame indefinitely.
fn unmaximize_settled(
    win: &adw::ApplicationWindow,
    last: &Rc<Cell<Option<[f64; 4]>>>,
    tick: u8,
) -> bool {
    if tick == 0 || win.is_maximized() {
        return false;
    }
    let Some(nswin) = crate::macos_window::nswindow_for_widget(win) else {
        return false;
    };
    let cur = frame_parts(nswin.frame());
    let stable = last.get() == Some(cur);
    last.set(Some(cur));
    stable && nswin.screen().map(|s| frame_parts(s.visibleFrame())).as_ref() != Some(&cur)
}

/// A deferred entry is still wanted: no newer legacy request superseded it (generation), no
/// native exit is in flight, and no native fullscreen state or in-flight native transition
/// owns the window — the AppKit fullscreen mask flips mid-transition, so the mask alone can
/// miss a native entry that already started; a settled-frame snapshot taken during the gap
/// would capture transitional geometry as the prior window state.
fn pending_entry_still_valid(win: &adw::ApplicationWindow, session: u32) -> bool {
    if SESSION.get() != session || crate::macos_fs_exit::exit_armed() {
        return false;
    }
    match crate::macos_window::nswindow_for_widget(win) {
        Some(nswin) => {
            !crate::macos_window::ns_window_is_native_fullscreen(&nswin)
                && !crate::macos_window::gdk_macos_in_fullscreen_transition(win)
        }
        // The NSWindow is already gone (teardown): nothing to defer an entry onto.
        None => false,
    }
}
