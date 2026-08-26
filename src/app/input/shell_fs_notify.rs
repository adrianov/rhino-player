// Handles owned by the `fullscreened_notify` closure (`w_in_fullscreen`), plus the
// enter / leave / reset steps it dispatches to.

/// Widget handles touched on every fullscreen notify.
struct FsNotifyWidgets {
    gl: gtk::GLArea,
    recent: gtk::Box,
    bottom: gtk::Box,
    fs_clock: gtk::Label,
    fs_tick_slot: Rc<RefCell<Option<glib::SourceId>>>,
}

impl FsNotifyWidgets {
    fn new(ctx: &WindowInputCtx) -> Self {
        Self {
            gl: ctx.shell.gl.clone(),
            recent: ctx.shell.recent.clone(),
            bottom: ctx.shell.bottom.clone(),
            fs_clock: ctx.fs_clock.clone(),
            fs_tick_slot: ctx.fs_clock_tick.clone(),
        }
    }
}

/// Transition-state slots driven across notify cycles.
struct FsNotifySlots {
    nav: Rc<RefCell<Option<glib::SourceId>>>,
    cur: Rc<RefCell<Option<glib::SourceId>>>,
    sq: Rc<Cell<Option<Instant>>>,
    lcap: Rc<Cell<Option<(f64, f64)>>>,
    lgl: Rc<Cell<Option<(f64, f64)>>>,
    fr: Rc<RefCell<Option<(i32, i32)>>>,
    lu: Rc<RefCell<(i32, i32)>>,
    skip: Rc<Cell<bool>>,
    pause_stash: Rc<RefCell<Option<bool>>>,
    fs_busy: Rc<Cell<bool>>,
    fs_settle: Rc<RefCell<Option<glib::SourceId>>>,
}

impl FsNotifySlots {
    fn new(ctx: &WindowInputCtx) -> Self {
        Self {
            nav: ctx.nav_t.clone(),
            cur: ctx.cur_t.clone(),
            sq: ctx.motion_squelch.clone(),
            lcap: ctx.last_cap_xy.clone(),
            lgl: ctx.last_gl_xy.clone(),
            fr: ctx.fs_restore.clone(),
            lu: ctx.last_unmax.clone(),
            skip: ctx.skip_max_to_fs.clone(),
            pause_stash: ctx.fs_pause_stash.clone(),
            fs_busy: Rc::clone(&ctx.fs_transition_busy),
            fs_settle: Rc::clone(&ctx.fs_transition_settle),
        }
    }
}

/// Cloned state shared by every fullscreened-notify invocation.
struct FsNotifyDeps {
    widgets: FsNotifyWidgets,
    slots: FsNotifySlots,
    player: Rc<RefCell<Option<MpvBundle>>>,
    bars_shown: Rc<Cell<bool>>,
    play: PlayToggleCtx,
    tch: Rc<dyn Fn(&adw::ApplicationWindow)>,
    #[cfg(target_os = "macos")]
    ch: Rc<ChromeBarHide>,
}

impl FsNotifyDeps {
    fn new(ctx: &WindowInputCtx, tch: Rc<dyn Fn(&adw::ApplicationWindow)>) -> Self {
        Self {
            widgets: FsNotifyWidgets::new(ctx),
            slots: FsNotifySlots::new(ctx),
            player: ctx.player.clone(),
            bars_shown: ctx.bar_show.clone(),
            play: ctx.play_toggle.clone(),
            tch,
            #[cfg(target_os = "macos")]
            ch: Rc::clone(&ctx.ch_hide),
        }
    }
}

/// Every notify first drops pending nav/cursor sources and pointer-position memory.
fn fs_notify_reset(deps: &FsNotifyDeps) {
    drop_glib_source(deps.slots.nav.as_ref());
    drop_glib_source(deps.slots.cur.as_ref());
    deps.slots.sq.set(None);
    deps.slots.lcap.set(None);
    deps.slots.lgl.set(None);
}

/// Enter may need to chain into maximize; honors the exit→re-entry deferral latch.
fn fs_notify_maybe_maximize(deps: &FsNotifyDeps, w: &adw::ApplicationWindow) {
    // Clear the leave-fullscreen deferral latch; stash first so we can still suppress a
    // redundant `maximize()` when this notify fires mid exit→re-enter AppKit turbulence.
    let defer_max_pair = deps.slots.skip.get();
    deps.slots.skip.set(false);
    // Only skip the paired `maximize` in that window — still run chrome / clock so a
    // true→false→true notify sequence does not leave stale UI if the platform emits one
    // during an AppKit transition.
    // Avoid synchronous `maximize()` in this notify on macOS: fullscreen transitions
    // can reconfigure GdkMacosMonitor's display link while frame callbacks are in
    // flight; `_gdk_macos_monitor_remove_frame_callback` may then call
    // `gdk_display_link_source_pause` when the new link is already paused (GDK CRITICAL:
    // `source->paused == FALSE`). Defer to the next main-loop turn.
    if !defer_max_pair && !w.is_maximized() {
        #[cfg(not(target_os = "macos"))]
        linux_fs_notify_maximize_now(&deps.slots.fr, w);
        #[cfg(target_os = "macos")]
        macos_fs_notify_defer_maximize(&deps.slots.fr, w);
    }
}

/// Enter branch: hide bars, stash pause state, start the wall clock, repaint chrome and
/// schedule the cursor hide.
fn fs_notify_enter(deps: &FsNotifyDeps, w: &adw::ApplicationWindow) {
    fs_notify_maybe_maximize(deps, w);
    deps.bars_shown.set(false);
    fs_on_enter_pause(&deps.play, deps.slots.pause_stash.as_ref());
    #[cfg(target_os = "macos")]
    popdown_header_menus(
        &[
            deps.ch.vol.clone(),
            deps.ch.sub.clone(),
            deps.ch.speed.clone(),
        ],
        "fullscreen_enter",
    );
    show_fs_wall_clock_fullscreen(&deps.widgets.fs_clock, &deps.widgets.fs_tick_slot, w);
    (deps.tch)(w);
    hide_cursor_after_bars_hide(w, &deps.widgets.gl, &deps.widgets.recent, &deps.player);
}

/// Shared leave prep: re-latch the skip flag, restore bars + clock + pointer immediately;
/// geometry/chrome restore is deferred below.
fn fs_leave_prep(deps: &FsNotifyDeps, w: &adw::ApplicationWindow) {
    deps.slots.skip.set(true);
    deps.bars_shown.set(true);
    stop_fs_clock_tick(&deps.widgets.fs_tick_slot);
    deps.widgets.fs_clock.set_visible(false);
    show_chrome_pointer(w, &deps.widgets.gl);
}

// Defer unmaximize + set_default_size: calling unmaximize synchronously from the fullscreened
// notify can leave `is_fullscreen()` true for one more notify cycle, which hits
// `maximized_notify`'s "!maximized && fullscreen" path → unfullscreen again and
// recurse until stack overflow (e.g. double-click exit).
//
// On macOS, `idle_add_once` still pumps mid `_NSExitFullScreenTransitionController`;
// `apply_chrome` touches traffic-light cells (`_NSThemeZoomWidgetCell`) and can recurse
// `_updateTitlebarContainerViewFrameIfNecessary` ↔ `_syncToolbarPosition` → stack overflow.
// Defer restore + chrome with [`crate::fullscreen_timing::TRANSITION_SETTLE`].
#[cfg(target_os = "macos")]
fn fs_notify_leave(deps: &FsNotifyDeps, w: &adw::ApplicationWindow, fs_leave_gen: &Rc<Cell<u32>>) {
    fs_leave_prep(deps, w);
    macos_schedule_leave_fs_restore_chrome(
        Rc::new(LeaveFsRestoreCtx {
            gen: Rc::clone(fs_leave_gen),
            want_gen: fs_leave_gen.get(),
            fr: Rc::clone(&deps.slots.fr),
            lu: Rc::clone(&deps.slots.lu),
            win: w.clone(),
            skip: Rc::clone(&deps.slots.skip),
            tch: Rc::clone(&deps.tch),
            play: deps.play.clone(),
            pause: Rc::clone(&deps.slots.pause_stash),
            polls: 0,
        }),
        crate::fullscreen_timing::TRANSITION_SETTLE,
    );
}

#[cfg(not(target_os = "macos"))]
fn fs_notify_leave(deps: &FsNotifyDeps, w: &adw::ApplicationWindow) {
    fs_leave_prep(deps, w);
    schedule_leave_fs_idle_linux(
        Rc::clone(&deps.slots.fr),
        Rc::clone(&deps.slots.lu),
        w.clone(),
        Rc::clone(&deps.slots.skip),
        Rc::clone(&deps.tch),
        deps.play.clone(),
        Rc::clone(&deps.slots.pause_stash),
    );
}
