//! macOS **legacy fullscreen** (IINA-style): the window frame covers a screen on the ordinary
//! window layer — no native fullscreen transition, no new Space. GTK `fullscreened` cannot be
//! reused: gdk-macos derives it from the AppKit style mask and drives fullscreen through
//! `toggleFullScreen:` (see `macos_window_fs.rs`), so this module owns its own state, publishes
//! it via [`active`] (consumed by the `window_fullscreened` gates in `app`), and replays the
//! fullscreened-notify enter/leave chrome steps through a notify hook registered by the
//! `fullscreened_notify` wiring (`shell_fullscreen.rs`).
//!
//! Reference: IINA `MainWindowController.legacyAnimateToFullscreen` / `legacyAnimateToWindowed`
//! — presentation-options auto-hide for menu bar + Dock, content-covering frame, floating level,
//! shadow off, exact-frame restore on exit.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::GtkWindowExt;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSApplicationPresentationOptions, NSFloatingWindowLevel, NSScreen, NSWindow,
    NSWindowButton, NSWindowLevel, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

type NotifyFn = Rc<dyn Fn(&adw::ApplicationWindow, bool)>;

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    /// Bumped on every legacy enter request and exit; pending frame re-asserts and deferred
    /// unmaximize entries capture it and abort when it moves, so a stale callback can never
    /// touch a later session's window. Native fullscreen transitions do NOT bump it — they
    /// invalidate through the busy checks instead (see `busy_for_reassert`).
    static SESSION: Cell<u32> = const { Cell::new(0) };
    static PRIOR_FRAME: Cell<[f64; 4]> = const { Cell::new([0.0; 4]) };
    static PRIOR_LEVEL: Cell<NSWindowLevel> = const { Cell::new(0) };
    static PRIOR_SHADOW: Cell<bool> = const { Cell::new(true) };
    static PRIOR_PRESENTATION: Cell<NSApplicationPresentationOptions> =
        const { Cell::new(NSApplicationPresentationOptions::empty()) };
    static PRIOR_STYLE_MASK: Cell<NSWindowStyleMask> =
        const { Cell::new(NSWindowStyleMask::empty()) };
    static NOTIFY: RefCell<Option<NotifyFn>> = const { RefCell::new(None) };
}

fn bump_session() {
    SESSION.set(SESSION.get().wrapping_add(1));
}

/// The transition generation: bumped on every legacy enter request and exit. Deferred
/// callbacks (leave chrome, cover applies, re-asserts) capture it and stand down when it
/// moves, so stale work can never consume a newer session's state.
pub fn session() -> u32 {
    SESSION.get()
}

/// Whether the shell is in legacy fullscreen (window content covers a screen while GTK/native
/// fullscreen is untouched). Drives the `window_fullscreened` gates.
pub fn active() -> bool {
    ACTIVE.get()
}

/// Register the GTK-side notify bridge. Called once from the `fullscreened_notify` wiring, which
/// owns the chrome state the hook replays (bars, pause stash, wall clock, cursor).
pub fn set_notify_hook(hook: NotifyFn) {
    NOTIFY.with(|slot| *slot.borrow_mut() = Some(hook));
}

fn notify(entering: bool, win: &adw::ApplicationWindow) {
    let hook = NOTIFY.with(|slot| slot.borrow().clone());
    if let Some(hook) = hook {
        hook(win, entering);
    }
}

fn frame_parts(f: NSRect) -> [f64; 4] {
    [f.origin.x, f.origin.y, f.size.width, f.size.height]
}

fn parts_frame(p: [f64; 4]) -> NSRect {
    NSRect::new(NSPoint::new(p[0], p[1]), NSSize::new(p[2], p[3]))
}

/// Capture **every** prior AppKit word the exit restores — frame, level, shadow, presentation
/// options, style mask — BEFORE the enter marks the session active: an exit during the deferred
/// cover apply restores these onto an unmodified window, so a late capture would race it and
/// paste a previous session's (or default) presentation/decorations over the live window.
fn save_prior_window_state(mtm: MainThreadMarker, nswin: &NSWindow) {
    PRIOR_FRAME.set(frame_parts(nswin.frame()));
    PRIOR_LEVEL.set(nswin.level());
    PRIOR_SHADOW.set(nswin.hasShadow());
    PRIOR_PRESENTATION.set(NSApplication::sharedApplication(mtm).presentationOptions());
    PRIOR_STYLE_MASK.set(nswin.styleMask());
}

fn settle_legacy_frame(win: &adw::ApplicationWindow) {
    crate::macos_window::request_gdk_surface_layout(win);
    crate::macos_window::invalidate_window_layers(win);
}

/// Restore the exact prior frame, level, shadow, style mask, and presentation options.
fn restore_legacy_appkit_state(mtm: MainThreadMarker, nswin: &NSWindow) {
    NSApplication::sharedApplication(mtm).setPresentationOptions(PRIOR_PRESENTATION.get());
    nswin.setLevel(PRIOR_LEVEL.get());
    nswin.setHasShadow(PRIOR_SHADOW.get());
    // Titled back on before the frame restore: the prior frame is on-screen geometry, so the
    // constrain pass is a no-op, and the mask is back before GDK observes the window again.
    nswin.setStyleMask(PRIOR_STYLE_MASK.get());
    nswin.setFrame_display(parts_frame(PRIOR_FRAME.get()), true);
}

/// Whether a native fullscreen state owns the window at legacy-exit time: its own leave
/// sequence already ran the chrome steps, so the saved legacy state is dropped silently
/// (always-on log — a dropped exit must be traceable).
fn exit_state_owned_by_native(nswin: &NSWindow) -> bool {
    let native = crate::macos_window::ns_window_is_native_fullscreen(nswin);
    if native {
        eprintln!("[rhino] legacy-fs: exit: native fullscreen owns the window — state dropped");
    }
    native
}

/// Cover the current screen, then run the enter notify steps. Entering from a maximized
/// (zoomed) window first unmaximizes and waits for the zoom-out to complete and settle, so the
/// saved prior frame is settled windowed geometry and no late zoom lands over the cover.
pub fn enter(win: &adw::ApplicationWindow) {
    if ACTIVE.get() || crate::macos_fs_exit::exit_armed() {
        return;
    }
    wire_level_swap_on_app_active(win);
    // Request-time generation bump: invalidates any pending unmaximize-wait entry from an
    // earlier request — this call (or a later exit) now owns the transition decision.
    bump_session();
    // gdk-macos drops the maximized flag when unmaximize is requested, while AppKit's
    // zoom-out may not even have started; a request in flight must go through the stability
    // wait even though is_maximized() already reads false.
    if win.is_maximized() || unmaximize_in_flight() {
        enter_after_unmaximize(win);
        return;
    }
    enter_now(win);
}

/// One-shot snapshot of the native traffic lights (window-base coordinates), window frame,
/// style mask, and toolbar presence — the lights are re-laid-out by AppKit whenever the
/// titled mask or frame flips, and a stale layout is the "displaced traffic lights" symptom.
fn log_window_chrome(nswin: &NSWindow, at: &str) {
    // objc2-app-kit does not export the Close/Miniaturize/Zoom button kinds as statics
    // (only NSWindowFullScreenButton); the numeric members are AppKit-stable (0/1/2).
    let (close_btn, mini_btn, zoom_btn) =
        (NSWindowButton(0), NSWindowButton(1), NSWindowButton(2));
    let mut buttons = String::new();
    for (name, kind) in [
        ("close", close_btn),
        ("mini", mini_btn),
        ("zoom", zoom_btn),
    ] {
        let Some(b) = nswin.standardWindowButton(kind) else {
            buttons.push_str(&format!("{name}=nil "));
            continue;
        };
        // `convertRect_toView(_, None)` expects the rect in the receiver's own coordinates —
        // `bounds()` — and converts to window base. `frame()` is superview-space and would
        // double-translate the origin.
        let bf = b.convertRect_toView(b.bounds(), None);
        buttons.push_str(&format!(
            "{name}={:.0},{:.0} {:.0}x{:.0} ",
            bf.origin.x, bf.origin.y, bf.size.width, bf.size.height
        ));
    }
    let f = nswin.frame();
    eprintln!(
        "[rhino] legacy-fs: chrome at={at} frame={:.0}x{:.0}+{:.0}+{:.0} mask={:?} toolbar={} {buttons}",
        f.size.width,
        f.size.height,
        f.origin.x,
        f.origin.y,
        nswin.styleMask(),
        nswin.toolbar().is_some(),
    );
}

fn enter_now(win: &adw::ApplicationWindow) {
    let Some((nswin, screen, prior, mtm)) = prepared_cover(win) else {
        return;
    };
    // Every prior word captured before the session goes active: an exit landing during the
    // deferred cover apply below must restore the live window's own values (a no-op), never a
    // previous session's presentation word or style mask.
    save_prior_window_state(mtm, &nswin);
    ACTIVE.set(true);
    bump_session();
    // Bars off BEFORE the frame jump: the gdk-macos surface rescales its last committed buffer
    // during the big resize, and a laid-out header in that buffer smears across the video as a
    // scaled mid-screen ghost. The chrome hide collapses instantly (no reveal animation); the
    // cover apply is deferred one commit slice so GTK relayouts and commits a bars-less buffer
    notify(true, win);
    log_window_chrome(&nswin, "enter");
    let session = SESSION.get();
    let w2 = win.clone();
    let nswin2 = nswin.clone();
    let _ = glib::source::timeout_add_local_once(
        crate::fullscreen_timing::BAR_HIDE_COMMIT,
        move || apply_cover_after_bar_hide(w2, nswin2, screen, prior, session, 0),
    );
}

/// Common tail of the legacy exit: restore the prior AppKit words, settle the GTK frame,
/// emit the byte-format restore line, and snapshot the native chrome for the traffic-light
/// layout probe.
fn restore_and_log_exit(mtm: MainThreadMarker, win: &adw::ApplicationWindow, nswin: &NSWindow) {
    let frame = parts_frame(PRIOR_FRAME.get());
    restore_legacy_appkit_state(mtm, nswin);
    settle_legacy_frame(win);
    eprintln!(
        "[rhino] legacy-fs: exit restore={}x{}+{}+{}",
        frame.size.width, frame.size.height, frame.origin.x, frame.origin.y,
    );
    log_window_chrome(nswin, "exit");
}

/// Restore the saved window state, then run the leave notify steps. Stale state while AppKit's
/// native fullscreen owns the window is dropped silently — the native machinery already ran its
/// own leave sequence.
pub fn exit(win: &adw::ApplicationWindow) {
    exit_with_transition_defer(win, 0, SESSION.get());
}

/// Settle-tick budget for waiting out a native transition before the exit decides (8 × 380 ms
/// ≈ 3 s — well past any real transition, tight enough that a stuck flag cannot wedge exit).
const TRANSITION_EXIT_DEFERS: u8 = 8;

/// The native fullscreen mask is not the only AppKit ownership word: during a native
/// transition (enter or exit) the mask may already be windowed while AppKit still reshapes the
/// window — restoring style and frame under it fights the transition. Defer the exit decision
/// on settle ticks until both the mask and the transition flag are clear. Bounded: once
/// `TRANSITION_EXIT_DEFERS` runs out, fall through to the native-mask decision with an
/// always-on log — a dropped exit must be traceable, and a wedged ACTIVE must not strand the
/// session.
///
/// Generation discipline: `want_gen` is captured at the original exit request and revalidated
/// by every retry — the module's staleness idiom (re-assert chains, unmaximize entries, cover
/// deferral all do the same). A later enter or completed exit bumps `SESSION`, so a stale
/// retry can never complete a session it does not own.
fn exit_with_transition_defer(win: &adw::ApplicationWindow, retries: u8, want_gen: u32) {
    if SESSION.get() != want_gen || !ACTIVE.get() {
        return;
    }
    if crate::macos_window::gdk_macos_in_fullscreen_transition(win) {
        if retries >= TRANSITION_EXIT_DEFERS {
            eprintln!(
                "[rhino] legacy-fs: exit: transition flag stuck after {retries} defers — deciding now"
            );
        } else {
            let w2 = win.clone();
            let _ = glib::source::timeout_add_local_once(
                crate::fullscreen_timing::TRANSITION_SETTLE,
                move || exit_with_transition_defer(&w2, retries + 1, want_gen),
            );
            return;
        }
    }
    if !ACTIVE.take() {
        return;
    }
    complete_exit(win);
}

/// Active-gated exit tail: bump the session (kills any remaining callback chain), then either
/// drop the state silently (native fullscreen owns the window) or restore the saved AppKit
/// words, schedule the windowed re-assert and run the leave notify.
fn complete_exit(win: &adw::ApplicationWindow) {
    bump_session();
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(nswin) = crate::macos_window::nswindow_for_widget(win) else {
        eprintln!("[rhino] legacy-fs: exit: no NSWindow — state dropped");
        return;
    };
    if exit_state_owned_by_native(&nswin) {
        return;
    }
    // A user live-resize from this moment on cancels the windowed re-assert chain scheduled
    // below (the user owns the frame now). Programmatic frame writes never post the AppKit
    // live-resize pair, so the restore itself cannot arm it.
    arm_user_resize_watch(&nswin);
    restore_and_log_exit(mtm, win, &nswin);
    schedule_frame_reassert(win, PRIOR_FRAME.get(), 0, 0, false);
    notify(false, win);
}

// Sibling units pulled in with `include!` — one flat module scope, split for size: the cover
// machinery (screen lookup + AppKit cover mutations + the deferred, native-aware cover apply),
// the frame re-assert machinery (session-generation-gated convergence), and the deferred
include!("macos_legacy_fs_cover.rs");
include!("macos_legacy_fs_reassert.rs");
include!("macos_legacy_fs_unmax.rs");
include!("macos_legacy_fs_levels.rs");
