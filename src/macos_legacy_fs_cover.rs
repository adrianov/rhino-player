// Cover machinery for the legacy-fullscreen state owner (`macos_legacy_fs`), pulled in with
// `include!` — one flat module scope, split for size. Screen lookup, the AppKit cover
// mutations, and the deferred cover apply: the bars-hide commit slice defers the frame jump,
// and a native fullscreen entry inside that window defers the apply further until the
// foreign owner releases the window. Imports come from the parent's flat scope.

/// AppKit mutations for legacy enter: presentation auto-hide (menu bar + Dock), floating level,
/// shadow off, content-covering frame. **All** prior state must already be saved
/// (`save_prior_window_state`) — the enter marks the session active before this runs, and an
/// exit during the deferral restores the saved word, so capturing here would race it.
///
/// The cover is derived here, AFTER the `Titled` bit is dropped, so the frame conversion never
/// includes a titlebar: for a borderless window the frame equals the content rect exactly
/// (`frameRectForContentRect` is identity), guaranteeing the cover never exceeds the screen.
/// Returns the cover frame actually applied (the caller logs it).
fn apply_legacy_fullscreen_appkit(
    mtm: MainThreadMarker,
    nswin: &NSWindow,
    screen: NSRect,
) -> NSRect {
    let app = NSApplication::sharedApplication(mtm);
    let mut opts = PRIOR_PRESENTATION.get();
    opts.insert(
        NSApplicationPresentationOptions::AutoHideDock
            | NSApplicationPresentationOptions::AutoHideMenuBar,
    );
    app.setPresentationOptions(opts);
    // Match the activation level swap (`on_app_active_changed`): the app may have resigned
    // activation during the bar-hide deferral — a floating cover then sits over the newly
    // active app's windows. The swap observer corrects it again on the next activation.
    nswin.setLevel(if app.isActive() {
        NSFloatingWindowLevel
    } else {
        NSNormalWindowLevel
    });
    nswin.setHasShadow(false);
    // The window server rounds the corners of titled windows (macOS 11+) — visible as rounded
    // video corners in the cover. Borderless windows are not rounded, so drop the Titled bit for
    // the cover and restore it on exit. Nothing draws from the titlebar while the chrome is
    // hidden, and `canBecomeKeyWindow` is overridden by the gdk backend.
    nswin.setStyleMask(PRIOR_STYLE_MASK.get() & !NSWindowStyleMask::Titled);
    let cover = nswin.frameRectForContentRect(screen);
    nswin.setFrame_display(cover, true);
    cover
}

/// The screen the window sits on (fallback: main screen). The cover conversion happens later,
/// under the final borderless style — see `apply_legacy_fullscreen_appkit`.
fn legacy_screen_frame(nswin: &NSWindow, mtm: MainThreadMarker) -> Option<NSRect> {
    Some(nswin.screen().or_else(|| NSScreen::mainScreen(mtm))?.frame())
}

/// Window, screen frame, prior-frame snapshot, and main-thread marker for the enter path; logs
/// the failure reason.
fn prepared_cover(
    win: &adw::ApplicationWindow,
) -> Option<(
    objc2::rc::Retained<NSWindow>,
    NSRect,
    [f64; 4],
    MainThreadMarker,
)> {
    let mtm = MainThreadMarker::new()?;
    let Some(nswin) = crate::macos_window::nswindow_for_widget(win) else {
        eprintln!("[rhino] legacy-fs: enter: no NSWindow");
        return None;
    };
    let Some(screen) = legacy_screen_frame(&nswin, mtm) else {
        eprintln!("[rhino] legacy-fs: enter: no NSScreen");
        return None;
    };
    Some((nswin.clone(), screen, frame_parts(nswin.frame()), mtm))
}
/// Deferred cover apply (see `enter_now`): the bars-hide commit slice has elapsed and this
/// session still owns the transition, so the cover frame lands and the result is logged.
///
/// A native fullscreen entry inside the commit window does not bump the session: before
/// applying, the foreign-owner state (`busy_for_reassert` — native fullscreen or a native
/// transition in flight) is revalidated and the apply defers on settle ticks until the owner
/// releases the window (bounded, like the frame re-assert budget). After the deferral budget,
/// the session is cancelled (`exit`) with an always-on log: an active legacy session whose
/// cover never landed would leave the user in hidden-chrome limbo with nothing fullscreen.
fn apply_cover_after_bar_hide(
    win: adw::ApplicationWindow,
    nswin: objc2::rc::Retained<NSWindow>,
    screen: NSRect,
    prior: [f64; 4],
    session: u32,
    defers: u8,
) {
    if SESSION.get() != session {
        return;
    }
    if busy_for_reassert(&win, &nswin) {
        if defers < BUSY_DEFERS {
            let w2 = win.clone();
            let nswin2 = nswin.clone();
            let _ = glib::source::timeout_add_local_once(
                crate::fullscreen_timing::TRANSITION_SETTLE,
                move || apply_cover_after_bar_hide(w2, nswin2, screen, prior, session, defers + 1),
            );
        } else {
            abandon_cover_apply(&win, &nswin, session, defers);
        }
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let cover = apply_legacy_fullscreen_appkit(mtm, &nswin, screen);
    settle_legacy_frame(&win);
    let applied = frame_parts(nswin.frame());
    eprintln!(
        "[rhino] legacy-fs: enter cover={}x{}+{}+{} applied={}x{}+{}+{} prior={}x{}+{}+{}",
        cover.size.width,
        cover.size.height,
        cover.origin.x,
        cover.origin.y,
        applied[2],
        applied[3],
        applied[0],
        applied[1],
        prior[2],
        prior[3],
        prior[0],
        prior[1],
    );
}

/// Budget exhausted while a foreign transition owns the window: log the abandonment and, when
/// this session is still current, cancel it. The enter already hid chrome, stashed pause and
/// started the clock, but the cover never landed — an active fullscreen state the user cannot
/// see must not linger. `exit` restores the untouched AppKit words, runs the leave notify
/// (bars/pointer/clock back, pause restored) and logs the exit; a native transition still
/// owning the window drops the state (its own machinery owns chrome from there). `exit` also
/// bumps the session, killing any remaining callback chain.
fn abandon_cover_apply(win: &adw::ApplicationWindow, nswin: &NSWindow, session: u32, defers: u8) {
    eprintln!(
        "[rhino] legacy-fs: cover apply abandoned busy defers={defers} native_fs={} transition={}",
        crate::macos_window::ns_window_is_native_fullscreen(nswin),
        crate::macos_window::gdk_macos_in_fullscreen_transition(win),
    );
    if SESSION.get() == session {
        exit(win);
    }
}
