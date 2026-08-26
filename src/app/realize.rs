#[derive(Clone)]
struct MpvRealizeCtx {
    shell: RealizeShell,
    st: RealizeState,
}

/// Window / widget handles the realize + render wiring touches.
#[derive(Clone)]
struct RealizeShell {
    player: Rc<RefCell<Option<MpvBundle>>>,
    app: adw::Application,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::Box,
    bottom: gtk::Box,
    close_video_btn: gtk::Button,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
}

/// Shared state, callbacks, and teardown actions captured at realize time.
#[derive(Clone)]
struct RealizeState {
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    bar_show: Rc<Cell<bool>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    file_boot: Rc<RefCell<Option<PathBuf>>>,
    win_aspect: Rc<WinAspectCell>,
    pending_recent_backfill: Rc<RefCell<Option<RecentBackfillJob>>>,
    close_video: gio::SimpleAction,
    move_to_trash: gio::SimpleAction,
    /// When set by [schedule_quit_persist], the next `GLArea::render` runs `teardown_gl_paint` then
    /// an idle calls [`MpvBundle::dispose_for_quit`] (`mpv_terminate_destroy`) and `quit`.
    mpv_teardown_after_draw: Rc<Cell<bool>>,
    playback_focus: Rc<Cell<bool>>,
    on_open_fail: Rc<dyn Fn(String)>,
}

fn gl_realize_bundle_ready(area: &gtk::GLArea, r: &MpvRealizeCtx, b: MpvBundle, auto_off: bool) {
    if auto_off {
        sync_smooth_60_to_off(&r.shell.app);
    }
    init_bundle_audio_and_subs(r, &b);
    *r.shell.player.borrow_mut() = Some(b);
    // macOS: when the recent grid (GTK overlay above the GLArea) becomes visible, hide
    // the native CAOpenGLLayer so the grid is not covered by the always-on-top video.
    if let Some(pl) = r.shell.player.borrow().as_ref() {
        pl.watch_overlay(&r.shell.recent);
    }
    drain_recent_backfill(&r.st.pending_recent_backfill);
    sync_close_video_action(
        &r.st.close_video,
        &r.shell.close_video_btn,
        &r.shell.player,
        &r.shell.recent,
    );
    sync_trash_action(&r.st.move_to_trash, &r.shell.player, &r.shell.recent);
    apply_bundle_sub_pos(area, r);
    if let Some(bundle) = r.shell.player.borrow_mut().as_mut() {
        let _ = bundle.mpv.disable_deprecated_events();
    }
    trigger_transport_install();
}
/// Applies saved volume / mute and subtitle prefs onto a freshly created bundle.
fn init_bundle_audio_and_subs(r: &MpvRealizeCtx, b: &MpvBundle) {
    let (av, am) = db::load_audio();
    let _ = b.mpv.set_property("volume", av);
    let _ = b.mpv.set_property("mute", am);
    let s = r.st.sub_pref.borrow();
    sub_prefs::apply_mpv(&b.mpv, &s);
}

fn apply_bundle_sub_pos(area: &gtk::GLArea, r: &MpvRealizeCtx) {
    if let Some(pl) = r.shell.player.borrow().as_ref() {
        let show = if r.shell.recent.is_visible() {
            true
        } else {
            r.st.bar_show.get()
        };
        sub_prefs::apply_sub_pos_for_toolbar(&pl.mpv, show, r.shell.bottom.height(), area.height());
    }
}

include!("realize_gl_handlers.rs");

/// Creates the libmpv render bundle when `GLArea` realizes, then wires drawing.
fn wire_mpv_realize(ctx: MpvRealizeCtx) {
    let attach_ctx = ctx.clone();
    let gl = attach_ctx.shell.gl.clone();
    let file_boot_rz = Rc::clone(&attach_ctx.st.file_boot);
    let vp_realize = Rc::clone(&attach_ctx.st.video_pref);
    gl.connect_realize(move |area| {
        mpv_gl_realize_attach(area, &attach_ctx, &file_boot_rz, &vp_realize);
    });
    wire_gl_render(
        &ctx.shell.gl,
        &ctx.shell.player,
        &ctx.st.mpv_teardown_after_draw,
        &ctx.shell.app,
        &ctx.shell.win,
    );
}

/// Wires the `GLArea` render handler that draws each frame and runs quit teardown.
fn wire_gl_render(
    gl: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    td: &Rc<Cell<bool>>,
    app: &adw::Application,
    win: &adw::ApplicationWindow,
) {
    #[cfg(target_os = "macos")]
    let _ = win;
    let p_draw = player.clone();
    let td = td.clone();
    let gl_bundle_drop = gl.clone();
    let app_rd = app.clone();
    #[cfg(not(target_os = "macos"))]
    let win_for_hide = Some(win.clone());
    #[cfg(target_os = "macos")]
    let win_for_hide: Option<adw::ApplicationWindow> = None;

    gl.connect_render(glib::clone!(
        #[strong]
        p_draw,
        #[strong]
        td,
        #[strong]
        gl_bundle_drop,
        #[strong]
        app_rd,
        #[strong]
        win_for_hide,
        move |area, _ctx| {
            mpv_gl_render_frame(
                area,
                &td,
                &p_draw,
                &app_rd,
                &gl_bundle_drop,
                win_for_hide.as_ref(),
            )
        }
    ));
}
