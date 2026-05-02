struct MpvRealizeCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::Box,
    bar_show: Rc<Cell<bool>>,
    bottom: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    file_boot: Rc<RefCell<Option<PathBuf>>>,
    win_aspect: Rc<Cell<Option<f64>>>,
    reapply_60: VideoReapply60,
    pending_recent_backfill: Rc<RefCell<Option<RecentBackfillJob>>>,
    close_video: gio::SimpleAction,
    move_to_trash: gio::SimpleAction,
    /// When set by [schedule_quit_persist], the next `GLArea::render` runs `teardown_gl_paint` then
    /// an idle calls [`MpvBundle::dispose_for_quit`] (`mpv_terminate_destroy`) and `quit`.
    mpv_teardown_after_draw: Rc<Cell<bool>>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
}

struct GlRealizeOkRefs {
    p_realize: Rc<RefCell<Option<MpvBundle>>>,
    vp_realize: Rc<RefCell<db::VideoPrefs>>,
    app_realize: adw::Application,
    sp_realize: Rc<RefCell<db::SubPrefs>>,
    recent_rz: gtk::Box,
    last_rz: Rc<RefCell<Option<PathBuf>>>,
    reapply_rz: VideoReapply60,
    pending_rz: Rc<RefCell<Option<RecentBackfillJob>>>,
    close_video: gio::SimpleAction,
    move_to_trash: gio::SimpleAction,
    bottom_rz: gtk::Box,
    bshow_rz: Rc<Cell<bool>>,
    win_rz: adw::ApplicationWindow,
    gl_rz: gtk::GLArea,
    on_vid_rz: Rc<dyn Fn()>,
    wa_st: Rc<Cell<Option<f64>>>,
    ol_rz: Rc<dyn Fn()>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
}

fn gl_realize_bundle_ready(
    area: &gtk::GLArea,
    r: &GlRealizeOkRefs,
    file_boot_rz: &Rc<RefCell<Option<PathBuf>>>,
    skip_preload_followups: bool,
    b: MpvBundle,
    auto_off: bool,
) {
    if auto_off {
        sync_smooth_60_to_off(&r.app_realize);
    }
    let (av, am) = db::load_audio();
    let _ = b.mpv.set_property("volume", av);
    let _ = b.mpv.set_property("mute", am);
    {
        let s = r.sp_realize.borrow();
        sub_prefs::apply_mpv(&b.mpv, &s);
    }
    *r.p_realize.borrow_mut() = Some(b);
    // macOS: when the recent grid (GTK overlay above the GLArea) becomes visible, hide
    // the native CAOpenGLLayer so the grid is not covered by the always-on-top video.
    if let Some(pl) = r.p_realize.borrow().as_ref() {
        pl.watch_overlay(&r.recent_rz);
    }
    let preload_auto_off =
        preload_first_continue(&r.p_realize, &r.vp_realize, &r.recent_rz, &r.last_rz);
    if preload_auto_off == Some(true) {
        sync_smooth_60_to_off(&r.app_realize);
    }
    if preload_auto_off.is_some() && !skip_preload_followups {
        schedule_preload_pause(r.p_realize.clone(), r.recent_rz.clone());
        schedule_preload_reapply_60(
            r.p_realize.clone(),
            r.reapply_rz.clone(),
            r.recent_rz.clone(),
            r.app_realize.clone(),
        );
    }
    drain_recent_backfill(&r.pending_rz);
    sync_close_video_action(&r.close_video, &r.p_realize, &r.recent_rz);
    sync_trash_action(&r.move_to_trash, &r.p_realize, &r.recent_rz);
    if let Some(pl) = r.p_realize.borrow().as_ref() {
        let show = if r.recent_rz.is_visible() {
            true
        } else {
            r.bshow_rz.get()
        };
        sub_prefs::apply_sub_pos_for_toolbar(
            &pl.mpv,
            show,
            r.bottom_rz.height(),
            area.height(),
        );
    }
    if let Some(bundle) = r.p_realize.borrow_mut().as_mut() {
        let _ = bundle.mpv.disable_deprecated_events();
    }
    trigger_transport_install();
    if let Some(p) = file_boot_rz.replace(None) {
        if let Err(e) = try_load(
            &p,
            &r.p_realize,
            &r.win_rz,
            &r.gl_rz,
            &r.recent_rz,
            &LoadOpts::replace_media(
                r.last_rz.clone(),
                Some(Rc::clone(&r.on_vid_rz)),
                Rc::clone(&r.wa_st),
                Some(Rc::clone(&r.ol_rz)),
                false,
                false,
                r.hdr_title_mirror.clone(),
            ),
        ) {
            eprintln!("[rhino] try_load (startup): {e}");
        }
    }
}

/// Creates the libmpv render bundle when `GLArea` realizes, then wires drawing.
fn wire_mpv_realize(ctx: MpvRealizeCtx) {
    let MpvRealizeCtx {
        player,
        sub_pref,
        video_pref,
        app,
        win,
        gl,
        recent,
        bar_show,
        bottom,
        last_path,
        on_video_chrome,
        on_file_loaded,
        file_boot,
        win_aspect,
        reapply_60,
        pending_recent_backfill,
        close_video,
        move_to_trash,
        mpv_teardown_after_draw,
        hdr_title_mirror,
    } = ctx;

    let p_realize = player.clone();
    let sp_realize = sub_pref.clone();
    let vp_realize = Rc::clone(&video_pref);
    let app_realize = app.clone();
    let win_rz = win.clone();
    let gl_rz = gl.clone();
    let recent_rz = recent.clone();
    let bshow_rz = bar_show.clone();
    let bottom_rz = bottom.clone();
    let last_rz = last_path.clone();
    let on_vid_rz = on_video_chrome.clone();
    let ol_rz = Rc::clone(&on_file_loaded);
    let file_boot_rz = Rc::clone(&file_boot);
    let wa_st = Rc::clone(&win_aspect);
    let reapply_rz = reapply_60.clone();
    let pending_rz = pending_recent_backfill.clone();
    let ok_refs = GlRealizeOkRefs {
        p_realize: p_realize.clone(),
        vp_realize: Rc::clone(&vp_realize),
        app_realize: app_realize.clone(),
        sp_realize: sp_realize.clone(),
        recent_rz: recent_rz.clone(),
        last_rz: last_rz.clone(),
        reapply_rz: reapply_rz.clone(),
        pending_rz: pending_rz.clone(),
        close_video: close_video.clone(),
        move_to_trash: move_to_trash.clone(),
        bottom_rz: bottom_rz.clone(),
        bshow_rz: bshow_rz.clone(),
        win_rz: win_rz.clone(),
        gl_rz: gl_rz.clone(),
        on_vid_rz: on_vid_rz.clone(),
        wa_st: wa_st.clone(),
        ol_rz: ol_rz.clone(),
        hdr_title_mirror: hdr_title_mirror.clone(),
    };
    gl.connect_realize(move |area| {
        area.make_current();
        // Preload fills the bundle with the first history item; optional `argv[1]` / portal open
        // runs `try_load` right after GL init and replaces that file — follow-up preload idles must
        // NOT duplicate transport Smooth 60 resync (`FileLoaded` / `path`), or multiple `vf`
        // teardown/rebuild bursts can freeze the GPU / main loop.
        let skip_preload_followups = file_boot_rz.borrow().is_some();
        let init = {
            let mut v = vp_realize.borrow_mut();
            MpvBundle::new(area, &mut v)
        };
        match init {
            Ok((b, auto_off)) => gl_realize_bundle_ready(
                area,
                &ok_refs,
                &file_boot_rz,
                skip_preload_followups,
                b,
                auto_off,
            ),
            Err(e) => eprintln!("[rhino] OpenGL / mpv: {e}"),
        }
    });

    let p_draw = player.clone();
    let td = mpv_teardown_after_draw;
    let gl_bundle_drop = gl.clone();
    #[cfg(not(target_os = "macos"))]
    let win_rd = win.clone();
    let app_rd = app.clone();
    gl.connect_render(move |area, _ctx| {
        area.make_current();
        if td.replace(false) {
            // Final paint runs here (`teardown_gl_paint`); an idle binds GL again then
            // [`MpvBundle::dispose_for_quit`] (`mpv_terminate_destroy` — not `mpv_destroy`).
            if let Some(b) = p_draw.borrow().as_ref() {
                b.teardown_gl_paint(area);
            }
            #[cfg(not(target_os = "macos"))]
            win_rd.set_visible(false);
            let to_drop = p_draw.borrow_mut().take();
            let app_q = app_rd.clone();
            let gl_q = gl_bundle_drop.clone();
            glib::idle_add_local_once(move || {
                gl_q.make_current();
                if let Some(b) = to_drop {
                    b.dispose_for_quit(&gl_q);
                }
                app_q.quit();
            });
            return glib::Propagation::Stop;
        }
        // macOS: video lives in a native CAOpenGLLayer underneath gdk's GTK sublayer,
        // so the GLArea itself must publish alpha=0 pixels for the video to show
        // through. Clear with a fully transparent color and let the GTK chrome (which
        // gdk renders into the same sublayer) stay opaque on top.
        #[cfg(target_os = "macos")]
        if let Some(b) = p_draw.borrow().as_ref() {
            b.clear_glarea_transparent();
        }
        #[cfg(not(target_os = "macos"))]
        if let Some(b) = p_draw.borrow().as_ref() {
            b.draw(area);
        }
        glib::Propagation::Stop
    });
}

fn schedule_preload_pause(
    player: Rc<RefCell<Option<MpvBundle>>>,
    recent: gtk::Box,
) {
    let _ = glib::timeout_add_local(Duration::from_millis(100), move || {
        if recent.is_visible() {
            if let Some(b) = player.borrow().as_ref() {
                let _ = b.mpv.set_property("pause", true);
            }
        }
        glib::ControlFlow::Break
    });
}

fn schedule_preload_reapply_60(
    player: Rc<RefCell<Option<MpvBundle>>>,
    reapply: VideoReapply60,
    recent: gtk::Box,
    app: adw::Application,
) {
    let _ = glib::idle_add_local_once(move || {
        if !recent.is_visible() {
            return;
        }
        if let Some(b) = player.borrow().as_ref() {
            let mut vp = reapply.vp.borrow_mut();
            if !vp.smooth_60 {
                return;
            }
            // Preload path: grid visible and paused — `apply_mpv_video` will not attach `vf` until
            // `pause=no`; play from a card goes through `sync_smooth_vf_on_pause_transition`.
            let off = video_pref::apply_mpv_video(b, &mut vp, None).smooth_auto_off
                || video_pref::reapply_60_if_still_missing(b, &mut vp).smooth_auto_off;
            if off {
                sync_smooth_60_to_off(&app);
            }
        }
    });
}

