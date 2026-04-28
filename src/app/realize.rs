struct MpvRealizeCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
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
    gl.connect_realize(move |area| {
        area.make_current();
        let init = {
            let mut v = vp_realize.borrow_mut();
            MpvBundle::new(area, &mut v)
        };
        match init {
            Ok((b, auto_off)) => {
                if auto_off {
                    sync_smooth_60_to_off(&app_realize);
                }
                let (av, am) = db::load_audio();
                let _ = b.mpv.set_property("volume", av);
                let _ = b.mpv.set_property("mute", am);
                {
                    let s = sp_realize.borrow();
                    sub_prefs::apply_mpv(&b.mpv, &s);
                }
                *p_realize.borrow_mut() = Some(b);
                let preload_auto_off = preload_first_continue(&p_realize, &vp_realize, &recent_rz, &last_rz);
                if preload_auto_off == Some(true) {
                    sync_smooth_60_to_off(&app_realize);
                }
                if preload_auto_off.is_some() {
                    schedule_preload_pause(p_realize.clone(), recent_rz.clone());
                    schedule_preload_reapply_60(
                        p_realize.clone(),
                        reapply_rz.clone(),
                        recent_rz.clone(),
                        app_realize.clone(),
                    );
                }
                drain_recent_backfill(&pending_rz);
                sync_close_video_action(&close_video, &p_realize, &recent_rz);
                sync_trash_action(&move_to_trash, &p_realize, &recent_rz);
                if let Some(pl) = p_realize.borrow().as_ref() {
                    let show = if recent_rz.is_visible() {
                        true
                    } else {
                        bshow_rz.get()
                    };
                    sub_prefs::apply_sub_pos_for_toolbar(
                        &pl.mpv,
                        show,
                        bottom_rz.height(),
                        area.height(),
                    );
                }
                if let Some(bundle) = p_realize.borrow_mut().as_mut() {
                    let _ = bundle.mpv.disable_deprecated_events();
                }
                trigger_transport_install();
                if let Some(p) = file_boot_rz.replace(None) {
                    if let Err(e) = try_load(
                        &p,
                        &p_realize,
                        &win_rz,
                        &gl_rz,
                        &recent_rz,
                        &LoadOpts {
                            record: true,
                            play_on_start: false,
                            last_path: last_rz.clone(),
                            on_start: Some(Rc::clone(&on_vid_rz)),
                            win_aspect: wa_st.clone(),
                            on_loaded: Some(Rc::clone(&ol_rz)),
                            reapply_60: Some(reapply_rz.clone()),
                            reset_speed_to_normal: false,
                        },
                    ) {
                        eprintln!("[rhino] try_load (startup): {e}");
                    }
                }
            }
            Err(e) => eprintln!("[rhino] OpenGL / mpv: {e}"),
        }
    });

    let p_draw = player.clone();
    gl.connect_render(move |area, _ctx| {
        area.make_current();
        if let Some(b) = p_draw.borrow().as_ref() {
            b.draw(area);
        }
        glib::Propagation::Stop
    });
}

fn schedule_preload_pause(
    player: Rc<RefCell<Option<MpvBundle>>>,
    recent: gtk::ScrolledWindow,
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
    recent: gtk::ScrolledWindow,
    app: adw::Application,
) {
    let _ = glib::idle_add_local_once(move || {
        if !recent.is_visible() { return; }
        if let Some(b) = player.borrow().as_ref() {
            let off = video_pref::reapply_60_if_still_missing(&b.mpv, &mut reapply.vp.borrow_mut());
            if off { sync_smooth_60_to_off(&app); }
        }
    });
}

