fn restore_windowed_size(
    fr: &Rc<RefCell<Option<(i32, i32)>>>,
    lu: &Rc<RefCell<(i32, i32)>>,
    w: &adw::ApplicationWindow,
) {
    use gtk::prelude::NativeExt;

    let dims = fr.borrow_mut().take().or(Some(*lu.borrow()));
    let Some((gw, gh)) = dims else { return };
    // Clear maximized state explicitly: after fullscreen the compositor may not match `is_maximized`.
    w.set_default_size(gw, gh);
    w.set_maximized(false);
    w.unmaximize();
    w.set_default_size(gw, gh);
    if let Some(surface) = w.surface() {
        surface.request_layout();
    }
    w.present();
}

#[cfg(not(target_os = "macos"))]
fn schedule_leave_fs_idle_linux(
    fr_leave: Rc<RefCell<Option<(i32, i32)>>>,
    lu_leave: Rc<RefCell<(i32, i32)>>,
    w_leave: adw::ApplicationWindow,
    skip_leave: Rc<Cell<bool>>,
    tch_leave: Rc<dyn Fn(&adw::ApplicationWindow)>,
    play_leave: PlayToggleCtx,
    pause_leave: Rc<RefCell<Option<bool>>>,
) {
    let _ = glib::source::idle_add_local_once(move || {
        fs_on_exit_pause(&play_leave, pause_leave.as_ref());
        restore_windowed_size(&fr_leave, &lu_leave, &w_leave);
        let w2 = w_leave;
        let skip2 = skip_leave;
        let tch2 = tch_leave;
        glib::source::idle_add_local_once(move || {
            skip2.set(false);
            tch2(&w2);
        });
    });
}

#[cfg(target_os = "macos")]
const MACOS_LEAVE_FS_POLL: std::time::Duration = std::time::Duration::from_millis(80);

#[cfg(target_os = "macos")]
const MACOS_LEAVE_FS_POLL_MAX: u8 = 12;

#[cfg(target_os = "macos")]
struct LeaveFsRestoreCtx {
    gen: Rc<Cell<u32>>,
    want_gen: u32,
    fr: Rc<RefCell<Option<(i32, i32)>>>,
    lu: Rc<RefCell<(i32, i32)>>,
    win: adw::ApplicationWindow,
    skip: Rc<Cell<bool>>,
    tch: Rc<dyn Fn(&adw::ApplicationWindow)>,
    play: PlayToggleCtx,
    pause: Rc<RefCell<Option<bool>>>,
    polls: u8,
}

#[cfg(target_os = "macos")]
fn macos_leave_fs_restore_tick(ctx: Rc<LeaveFsRestoreCtx>) {
    if ctx.gen.get() != ctx.want_gen {
        ctx.skip.set(false);
        return;
    }
    if crate::macos_window::window_still_fullscreen(&ctx.win) && ctx.polls < MACOS_LEAVE_FS_POLL_MAX
    {
        let next = Rc::new(LeaveFsRestoreCtx {
            gen: Rc::clone(&ctx.gen),
            want_gen: ctx.want_gen,
            fr: Rc::clone(&ctx.fr),
            lu: Rc::clone(&ctx.lu),
            win: ctx.win.clone(),
            skip: Rc::clone(&ctx.skip),
            tch: Rc::clone(&ctx.tch),
            play: ctx.play.clone(),
            pause: Rc::clone(&ctx.pause),
            polls: ctx.polls.saturating_add(1),
        });
        let _ = glib::timeout_add_local_once(MACOS_LEAVE_FS_POLL, move || {
            macos_leave_fs_restore_tick(next);
        });
        return;
    }
    crate::macos_fs_exit::clear_exit();
    fs_on_exit_pause(&ctx.play, ctx.pause.as_ref());
    restore_windowed_size(&ctx.fr, &ctx.lu, &ctx.win);
    let w2 = ctx.win.clone();
    let skip2 = Rc::clone(&ctx.skip);
    let tch2 = Rc::clone(&ctx.tch);
    let _ = glib::source::idle_add_local_once(move || {
        skip2.set(false);
        tch2(&w2);
    });
}

#[cfg(target_os = "macos")]
fn macos_schedule_leave_fs_restore_chrome(
    gen: &Rc<Cell<u32>>,
    delay: std::time::Duration,
    want_gen: u32,
    fr_leave: Rc<RefCell<Option<(i32, i32)>>>,
    lu_leave: Rc<RefCell<(i32, i32)>>,
    w_leave: adw::ApplicationWindow,
    skip_leave: Rc<Cell<bool>>,
    tch_leave: Rc<dyn Fn(&adw::ApplicationWindow)>,
    play_leave: PlayToggleCtx,
    pause_leave: Rc<RefCell<Option<bool>>>,
) {
    let ctx = Rc::new(LeaveFsRestoreCtx {
        gen: Rc::clone(gen),
        want_gen,
        fr: fr_leave,
        lu: lu_leave,
        win: w_leave,
        skip: skip_leave,
        tch: tch_leave,
        play: play_leave,
        pause: pause_leave,
        polls: 0,
    });
    let ctx2 = Rc::clone(&ctx);
    let _ = glib::timeout_add_local_once(delay, move || macos_leave_fs_restore_tick(ctx2));
}
