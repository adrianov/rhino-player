struct FileLoadedCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    sibling_nav: SiblingNavUi,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    gl: gtk::GLArea,
    bar_show: Rc<Cell<bool>>,
    recent: gtk::ScrolledWindow,
    bottom: gtk::Box,
    sub_menu: gtk::MenuButton,
    close_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    trash_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    speed_sync: Rc<Cell<bool>>,
    speed_list: gtk::ListBox,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
}

fn make_file_loaded_handler(ctx: FileLoadedCtx) -> Rc<dyn Fn()> {
    let FileLoadedCtx {
        player,
        last_path,
        sibling_seof,
        sibling_nav,
        sub_pref,
        gl,
        bar_show,
        recent,
        bottom,
        sub_menu,
        close_action_cell,
        trash_action_cell,
        speed_sync,
        speed_list,
        video_pref,
        app,
    } = ctx;
    Rc::new({
        let p = player.clone();
        let lp = last_path.clone();
        let seof = sibling_seof.clone();
        let nav = sibling_nav.clone();
        let sp = sub_pref.clone();
        let g2 = gl.clone();
        let bshow = bar_show.clone();
        let rec = recent.clone();
        let bot = bottom.clone();
        let sub_m_btn = sub_menu.clone();
        let close_a = Rc::clone(&close_action_cell);
        let trash_a = Rc::clone(&trash_action_cell);
        let syf = speed_sync.clone();
        let sl = speed_list.clone();
        let vp_onload = Rc::clone(&video_pref);
        let app_onload = app.clone();
        move || {
            let cur = p
                .borrow()
                .as_ref()
                .and_then(|b| local_file_from_mpv(&b.mpv))
                .or_else(|| lp.borrow().clone());
            nav.refresh(cur.as_deref(), seof.as_ref());
            let p2 = p.clone();
            let sp2 = sp.clone();
            let g3 = g2.clone();
            let b3 = bshow.clone();
            let r3 = rec.clone();
            let bot2 = bot.clone();
            let sub320 = sub_m_btn.clone();
            let close_a2 = Rc::clone(&close_a);
            let trash_a2 = Rc::clone(&trash_a);
            let syf320 = syf.clone();
            let sl320 = sl.clone();
            let vp_320 = Rc::clone(&vp_onload);
            let app_320 = app_onload.clone();
            let _ = glib::timeout_add_local(Duration::from_millis(320), move || {
                if let Some(b) = p2.borrow().as_ref() {
                    schedule_sub_button_scan(p2.clone(), sub320.clone());
                    let pr = sp2.borrow();
                    sub_prefs::apply_mpv(&b.mpv, &pr);
                    let show = if r3.is_visible() { true } else { b3.get() };
                    sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, bot2.height(), g3.height());
                    audio_tracks::restore_saved_audio(&b.mpv);
                    audio_tracks::ensure_playable_audio(&b.mpv);
                    sub_tracks::autopick_sub_track(&b.mpv, &pr);
                    let listed = playback_speed::sync_list(&b.mpv, &syf320, &sl320);
                    let mut g = vp_320.borrow_mut();
                    if g.smooth_60 {
                        let off = if let Some(s) = listed {
                            video_pref::refresh_smooth_for_playback_speed(&b.mpv, &mut g, Some(s))
                        } else if video_pref::needs_playback_speed_env_resync(&b.mpv) {
                            video_pref::refresh_smooth_for_playback_speed(&b.mpv, &mut g, None)
                        } else {
                            video_pref::resync_smooth_if_speed_mismatch(&b.mpv, &mut g)
                        };
                        if off {
                            sync_smooth_60_to_off(&app_320);
                        }
                    }
                }
                if let Some(a) = close_a2.borrow().as_ref() {
                    sync_close_video_action(a, &p2, &r3);
                }
                if let Some(a) = trash_a2.borrow().as_ref() {
                    sync_trash_action(a, &p2, &r3);
                }
                glib::ControlFlow::Break
            });
            // 60p: [try_load] chains a second idle to [reapply_60_if_still_missing]. This 320ms hook
            // catches list snap and [vf] vs [mvtools_vf_eligible] in one pass.
        }
    })
}

struct SubStyleCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    gl: gtk::GLArea,
    bar_show: Rc<Cell<bool>>,
    recent: gtk::ScrolledWindow,
    bottom: gtk::Box,
    sub_scale_adj: gtk::Adjustment,
    sub_color_btn: gtk::ColorDialogButton,
}

fn wire_sub_style_controls(ctx: SubStyleCtx) {
    let SubStyleCtx {
        player,
        sub_pref,
        gl: gl_area,
        bar_show,
        recent,
        bottom,
        sub_scale_adj,
        sub_color_btn,
    } = ctx;
    {
        let p = player.clone();
        let sp = sub_pref.clone();
        let gll = gl_area.clone();
        let adj = sub_scale_adj.clone();
        let bshow = bar_show.clone();
        let rec = recent.clone();
        let bot = bottom.clone();
        sub_scale_adj.connect_value_changed(move |_| {
            let v = adj.value();
            sp.borrow_mut().scale = v;
            if let Some(b) = p.borrow().as_ref() {
                let pr = sp.borrow();
                sub_prefs::apply_mpv(&b.mpv, &pr);
                let show = if rec.is_visible() { true } else { bshow.get() };
                sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, bot.height(), gll.height());
            }
            db::save_sub(&sp.borrow());
            gll.queue_render();
        });
    }
    {
        let p = player.clone();
        let sp = sub_pref.clone();
        let gll = gl_area.clone();
        let btn = sub_color_btn.clone();
        let bshow = bar_show.clone();
        let rec = recent.clone();
        let bot = bottom.clone();
        sub_color_btn.connect_rgba_notify(move |_| {
            sp.borrow_mut().color = sub_prefs::rgba_to_u32(&btn.rgba());
            if let Some(b) = p.borrow().as_ref() {
                let pr = sp.borrow();
                sub_prefs::apply_mpv(&b.mpv, &pr);
                let show = if rec.is_visible() { true } else { bshow.get() };
                sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, bot.height(), gll.height());
            }
            db::save_sub(&sp.borrow());
            gll.queue_render();
        });
    }
}
