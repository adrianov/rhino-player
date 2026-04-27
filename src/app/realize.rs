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
                let preload_auto_off = preload_first_continue(&p_realize, &vp_realize, &recent_rz);
                if preload_auto_off == Some(true) {
                    sync_smooth_60_to_off(&app_realize);
                }
                if preload_auto_off.is_some() {
                    let p_pause = p_realize.clone();
                    let r_pause = recent_rz.clone();
                    let _ = glib::timeout_add_local(Duration::from_millis(100), move || {
                        if r_pause.is_visible() {
                            if let Some(b) = p_pause.borrow().as_ref() {
                                let _ = b.mpv.set_property("pause", true);
                            }
                        }
                        glib::ControlFlow::Break
                    });
                    let p_60 = p_realize.clone();
                    let r_60 = reapply_rz.clone();
                    let rec_60 = recent_rz.clone();
                    let app_60 = app_realize.clone();
                    let _ = glib::idle_add_local_once(move || {
                        if !rec_60.is_visible() {
                            return;
                        }
                        if let Some(b) = p_60.borrow().as_ref() {
                            let off = {
                                let mut g = r_60.vp.borrow_mut();
                                video_pref::reapply_60_if_still_missing(&b.mpv, &mut g)
                            };
                            if off {
                                sync_smooth_60_to_off(&app_60);
                            }
                        }
                    });
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

struct TransportPollCtx {
    app: adw::Application,
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    exit_after_current: Rc<Cell<bool>>,
    win_aspect: Rc<Cell<Option<f64>>>,
    idle_inhib: Rc<RefCell<Option<u32>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    reapply_60: VideoReapply60,
    seek_state: Rc<seek_bar_preview::SeekPreviewState>,
    speed_menu: gtk::MenuButton,
    seek: gtk::Scale,
    seek_adj: gtk::Adjustment,
    seek_sync: Rc<Cell<bool>>,
    time_left: gtk::Label,
    time_right: gtk::Label,
    play_pause: gtk::Button,
    wrap_prev: gtk::Box,
    wrap_next: gtk::Box,
    btn_prev: gtk::Button,
    btn_next: gtk::Button,
    vol_menu: gtk::MenuButton,
    vol_adj: gtk::Adjustment,
    vol_mute: gtk::ToggleButton,
    vol_sync: Rc<Cell<bool>>,
}

/// Keeps transport widgets in sync with mpv state; legacy timer until mpv event wiring replaces it.
fn start_transport_poll(ctx: TransportPollCtx) {
    let TransportPollCtx {
        app,
        player,
        sub_pref,
        win,
        gl,
        recent,
        last_path,
        sibling_seof,
        exit_after_current,
        win_aspect,
        idle_inhib,
        on_video_chrome,
        on_file_loaded,
        reapply_60,
        seek_state,
        speed_menu,
        seek,
        seek_adj,
        seek_sync,
        time_left,
        time_right,
        play_pause,
        wrap_prev,
        wrap_next,
        btn_prev,
        btn_next,
        vol_menu,
        vol_adj,
        vol_mute,
        vol_sync,
    } = ctx;

    let tw_l = time_left.downgrade();
    let tw_r = time_right.downgrade();
    let ppw = play_pause.downgrade();
    let wpw_prev = wrap_prev.downgrade();
    let wpw_next = wrap_next.downgrade();
    let bpw_prev = btn_prev.downgrade();
    let bpw_next = btn_next.downgrade();
    let spdm = speed_menu.downgrade();
    glib::timeout_add_local(
        Duration::from_millis(200),
        glib::clone!(
            #[strong]
            app,
            #[strong]
            player,
            #[strong]
            sub_pref,
            #[strong]
            win,
            #[strong]
            gl,
            #[strong]
            recent,
            #[strong]
            last_path,
            #[strong]
            sibling_seof,
            #[strong]
            exit_after_current,
            #[strong]
            idle_inhib,
            #[strong]
            on_video_chrome,
            #[strong]
            win_aspect,
            #[strong]
            on_file_loaded,
            #[strong]
            reapply_60,
            move || {
                maybe_advance_sibling_on_eof(
                    &player,
                    &win,
                    &gl,
                    &recent,
                    &last_path,
                    sibling_seof.as_ref(),
                    &exit_after_current,
                    &app,
                    &sub_pref,
                    &idle_inhib,
                    &on_video_chrome,
                    Rc::clone(&win_aspect),
                    Some(Rc::clone(&on_file_loaded)),
                    &reapply_60,
                );
                let Some(tl) = tw_l.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let Some(tr) = tw_r.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                seek_state.on_tick();
                let g = player.borrow();
                let Some(pl) = g.as_ref() else {
                    sibling_seof.clear_nav_sensitivity();
                    if let Some(pp) = ppw.upgrade() {
                        pp.set_sensitive(false);
                        pp.set_icon_name("media-playback-start-symbolic");
                        pp.set_tooltip_text(Some("No media"));
                    }
                    if let Some(w) = wpw_prev.upgrade() {
                        w.set_tooltip_text(Some("No media"));
                    }
                    if let Some(p) = bpw_prev.upgrade() {
                        p.set_sensitive(false);
                        p.set_can_target(false);
                    }
                    if let Some(w) = wpw_next.upgrade() {
                        w.set_tooltip_text(Some("No media"));
                    }
                    if let Some(n) = bpw_next.upgrade() {
                        n.set_sensitive(false);
                        n.set_can_target(false);
                    }
                    if let Some(sb) = spdm.upgrade() {
                        sb.set_sensitive(false);
                    }
                    return glib::ControlFlow::Continue;
                };
                sync_window_aspect_from_mpv(&pl.mpv, win_aspect.as_ref());
                let pos = pl.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
                let dur = pl.mpv.get_property::<f64>("duration").unwrap_or(0.0);
                tl.set_label(&format_time(pos));
                tr.set_label(&format_time(dur));
                let nav = TransportButtonRefs {
                    ppw: &ppw,
                    bpw_prev: &bpw_prev,
                    bpw_next: &bpw_next,
                    wpw_prev: &wpw_prev,
                    wpw_next: &wpw_next,
                    last_path: &last_path,
                    sibling_seof: sibling_seof.as_ref(),
                };
                sync_transport_buttons(pl, dur, &nav);
                let seek_vol = SeekVolumeRefs {
                    seek: &seek,
                    adj: &seek_adj,
                    seek_sync: &seek_sync,
                    spdm: &spdm,
                    vol_menu: &vol_menu,
                    vol_adj: &vol_adj,
                    vol_mute: &vol_mute,
                    vol_sync: &vol_sync,
                };
                sync_seek_and_volume(pl, pos, dur, &seek_vol);
                glib::ControlFlow::Continue
            }
        ),
    );
}

struct TransportButtonRefs<'a> {
    ppw: &'a glib::WeakRef<gtk::Button>,
    bpw_prev: &'a glib::WeakRef<gtk::Button>,
    bpw_next: &'a glib::WeakRef<gtk::Button>,
    wpw_prev: &'a glib::WeakRef<gtk::Box>,
    wpw_next: &'a glib::WeakRef<gtk::Box>,
    last_path: &'a Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: &'a SiblingEofState,
}

fn sync_transport_buttons(pl: &MpvBundle, dur: f64, refs: &TransportButtonRefs<'_>) {
    if let Some(pp) = refs.ppw.upgrade() {
        let has_media = dur > 0.0;
        pp.set_sensitive(has_media);
        if has_media && !pl.mpv.get_property::<bool>("pause").unwrap_or(false) {
            pp.set_icon_name("media-playback-pause-symbolic");
            pp.set_tooltip_text(Some("Pause (Space)"));
        } else {
            pp.set_icon_name("media-playback-start-symbolic");
            pp.set_tooltip_text(Some(if has_media {
                "Play (Space)"
            } else {
                "No media"
            }));
        }
    }
    let cur = if dur > 0.0 {
        local_file_from_mpv(&pl.mpv).or_else(|| refs.last_path.borrow().clone())
    } else {
        None
    };
    let (can_prev, can_next) = if let Some(c) = cur.as_ref().filter(|p| p.is_file()) {
        refs.sibling_seof.nav_sensitivity(c)
    } else {
        refs.sibling_seof.clear_nav_sensitivity();
        (false, false)
    };
    if let Some(p) = refs.bpw_prev.upgrade() {
        p.set_sensitive(can_prev);
        p.set_can_target(can_prev);
    }
    if let Some(w) = refs.wpw_prev.upgrade() {
        let tip = sibling_bar_tooltip(true, can_prev, cur.as_deref());
        w.set_tooltip_text(Some(tip.as_str()));
    }
    if let Some(n) = refs.bpw_next.upgrade() {
        n.set_sensitive(can_next);
        n.set_can_target(can_next);
    }
    if let Some(w) = refs.wpw_next.upgrade() {
        let tip = sibling_bar_tooltip(false, can_next, cur.as_deref());
        w.set_tooltip_text(Some(tip.as_str()));
    }
}

struct SeekVolumeRefs<'a> {
    seek: &'a gtk::Scale,
    adj: &'a gtk::Adjustment,
    seek_sync: &'a Rc<Cell<bool>>,
    spdm: &'a glib::WeakRef<gtk::MenuButton>,
    vol_menu: &'a gtk::MenuButton,
    vol_adj: &'a gtk::Adjustment,
    vol_mute: &'a gtk::ToggleButton,
    vol_sync: &'a Rc<Cell<bool>>,
}

fn sync_seek_and_volume(pl: &MpvBundle, pos: f64, dur: f64, refs: &SeekVolumeRefs<'_>) {
    let has_media = dur > 0.0;
    refs.seek.set_sensitive(has_media);
    if let Some(sb) = refs.spdm.upgrade() {
        sb.set_sensitive(has_media);
    }
    if has_media {
        refs.adj.set_lower(0.0);
        refs.adj.set_upper(dur);
        refs.seek_sync.set(true);
        refs.adj.set_value(pos.clamp(0.0, dur));
        refs.seek_sync.set(false);
    }

    let vol = pl.mpv.get_property::<f64>("volume").unwrap_or(0.0);
    let muted = pl.mpv.get_property::<bool>("mute").unwrap_or(false);
    refs.vol_menu.set_icon_name(vol_icon(muted, vol));
    if refs.vol_menu.is_active() {
        return;
    }
    let vmax = pl.mpv.get_property::<f64>("volume-max").unwrap_or(100.0);
    if vmax.is_finite() && vmax > 0.0 {
        refs.vol_adj.set_upper(vmax);
    }
    refs.vol_sync.set(true);
    refs.vol_adj.set_value(vol.clamp(0.0, refs.vol_adj.upper()));
    if refs.vol_mute.is_active() != muted {
        refs.vol_mute.set_active(muted);
    }
    refs.vol_mute.set_icon_name(vol_mute_pop_icon(muted));
    refs.vol_mute
        .set_tooltip_text(Some(if muted { "Unmute" } else { "Mute" }));
    refs.vol_sync.set(false);
}
