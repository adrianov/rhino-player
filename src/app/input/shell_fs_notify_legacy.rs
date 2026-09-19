// macOS legacy-fullscreen leave glue and the shared chrome-collapse animation suppression,
// pulled in with `include!` from `shell.rs` — one flat module scope with `shell_fs_notify.rs`,
// split for module size. The enter-chrome and generic leave steps stay in `shell_fs_notify.rs`.

thread_local! {
    /// Pending animation-suppression depth with the pre-suppression setting, saved once when
    /// the depth first rises above zero: overlapping chrome collapses (an enter notify
    /// replayed while an earlier suppression's restore idle is still pending) must not save
    /// the already-disabled `false` and restore it last, which would leave GTK animations
    /// disabled display-wide.
    static ANIM_SUPPRESS: std::cell::RefCell<(u32, bool)> = const {
        std::cell::RefCell::new((0, true))
    };
}

/// Enter one animation-suppression request: the display-wide setting is saved only when the
/// depth first rises above zero, so a nested request cannot capture the disabled value.
fn anim_suppress_enter(settings: &gtk::Settings) {
    ANIM_SUPPRESS.with(|slot| {
        let (depth, prev) = &mut *slot.borrow_mut();
        if *depth == 0 {
            *prev = settings.is_gtk_enable_animations();
        }
        *depth += 1;
    });
}

/// Finish one animation-suppression request; the saved setting is restored exactly once, when
/// the last pending request finishes.
fn anim_suppress_exit_restore(settings: &gtk::Settings) {
    ANIM_SUPPRESS.with(|slot| {
        let (depth, prev) = &mut *slot.borrow_mut();
        *depth = depth.saturating_sub(1);
        if *depth == 0 {
            settings.set_gtk_enable_animations(*prev);
        }
    });
}

/// Run the chrome hide with GTK widget animations disabled for this turn: the ToolbarView
/// reveal collapse lands immediately, so no bar pixels remain for the fullscreen frame jump
/// to rescale into a mid-screen ghost (legacy full screen). Re-enabled on the next idle once
/// every pending suppression has finished.
fn collapse_chrome_without_animation(w: &adw::ApplicationWindow, hide: impl FnOnce()) {
    let settings = gtk::Settings::for_display(&gtk::prelude::WidgetExt::display(w));
    anim_suppress_enter(&settings);
    settings.set_gtk_enable_animations(false);
    hide();
    glib::source::idle_add_local_once(move || anim_suppress_exit_restore(&settings));
}

/// Whether a deferred legacy-leave callback is stale: a newer legacy session bumped the
/// transition generation, or a native fullscreen state owns the shell (a native enter stashes
/// its own pause state — a stale leave consuming it would corrupt the new session's playback).
#[cfg(target_os = "macos")]
fn legacy_leave_stale(win: &adw::ApplicationWindow, session: u32) -> bool {
    crate::macos_legacy_fs::session() != session || win.is_fullscreen()
}

/// macOS legacy fullscreen leave: bars + clock + pointer restore immediately (no native
/// transition controller to wait out); pause + chrome restore after the frame restore settles.
/// Geometry restore is AppKit-side — exact prior frame in [`crate::macos_legacy_fs::exit`].
///
/// Both deferred callbacks validate the transition generation first: a legacy re-enter bumps
/// [`crate::macos_legacy_fs::session`] and stashes its own pause state, and a native enter
/// stashes pause too — a stale leave consuming that stash would corrupt the new session's
/// playback state.
#[cfg(target_os = "macos")]
fn legacy_fs_leave(deps: &Rc<FsNotifyDeps>, w: &adw::ApplicationWindow) {
    let session = crate::macos_legacy_fs::session();
    deps.bars_shown.set(true);
    stop_fs_clock_tick(&deps.widgets.fs_tick_slot);
    deps.widgets.fs_clock.set_visible(false);
    show_chrome_pointer(w, &deps.widgets.gl);
    schedule_legacy_fs_leave_restore(deps, w, session);
}

/// Deferred leave body (see [`legacy_fs_leave`]): generation-validated pause + chrome restore
/// at the next idle, then a final chrome touch after the frame restore settles.
#[cfg(target_os = "macos")]
fn schedule_legacy_fs_leave_restore(
    deps: &Rc<FsNotifyDeps>,
    w: &adw::ApplicationWindow,
    session: u32,
) {
    let d = Rc::clone(deps);
    let w2 = w.clone();
    let _ = glib::source::idle_add_local_once(move || {
        if legacy_leave_stale(&w2, session) {
            return;
        }
        fs_on_exit_pause(&d.play, d.slots.pause_stash.as_ref());
        (d.tch)(&w2);
        let d2 = Rc::clone(&d);
        let w3 = w2.clone();
        let _ = glib::timeout_add_local_once(
            crate::fullscreen_timing::TRANSITION_SETTLE,
            move || {
                if legacy_leave_stale(&w3, session) {
                    return;
                }
                (d2.tch)(&w3);
            },
        );
    });
}
