fn wire_recent_spacer_fullscreen(
    sp_empty: [gtk::Box; 4],
    win: &adw::ApplicationWindow,
    fs_restore: &Rc<RefCell<Option<(i32, i32)>>>,
    last_unmax: &Rc<RefCell<(i32, i32)>>,
    skip_max_to_fs: &Rc<Cell<bool>>,
    recent: &gtk::ScrolledWindow,
) {
    for sp in sp_empty {
        let d2 = gtk::GestureClick::new();
        d2.set_button(gtk::gdk::BUTTON_PRIMARY);
        let w2 = win.clone();
        let fr2 = fs_restore.clone();
        let lu2 = last_unmax.clone();
        let sk2 = skip_max_to_fs.clone();
        let rec2 = recent.clone();
        d2.connect_pressed(move |gest, n_press, _, _| {
            if n_press != 2 || !rec2.is_visible() {
                return;
            }
            let _ = gest.set_state(gtk::EventSequenceState::Claimed);
            toggle_fullscreen(&w2, &fr2, &lu2, &sk2);
        });
        sp.add_controller(d2);
    }
}

#[derive(Clone)]
struct PlayToggleCtx {
    app: adw::Application,
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    sub_menu: Option<gtk::MenuButton>,
    /// Bottom-bar play/pause button. The toggle handler updates its icon
    /// optimistically so the click feels instant; the 1 Hz transport tick
    /// reconciles with mpv's actual state right after.
    play_pause: gtk::Button,
}

fn toggle_play_pause(ctx: &PlayToggleCtx) -> bool {
    let g = ctx.player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    if b.mpv.get_property::<f64>("duration").unwrap_or(0.0) <= 0.0 {
        return false;
    }
    if ctx.recent.is_visible() {
        if let Some(path) = local_file_from_mpv(&b.mpv) {
            *ctx.last_path.borrow_mut() = std::fs::canonicalize(&path).ok();
            ctx.win.set_title(Some(title_for_open_path(&path).as_str()));
        }
        sync_window_aspect_from_mpv(&b.mpv, ctx.win_aspect.as_ref());
        resync_warm_continue(&b.mpv);
        ctx.gl.queue_render();
        drop(g);
        schedule_warm_reveal(ctx.clone());
        return true;
    }
    let paused = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    if paused {
        let off = {
            let mut pref = ctx.video_pref.borrow_mut();
            video_pref::resync_smooth_if_speed_mismatch(&b.mpv, &mut pref)
        };
        if off {
            sync_smooth_60_to_off(&ctx.app);
        }
    }
    if b.mpv.set_property("pause", !paused).is_ok() {
        flip_play_icon(&ctx.play_pause, !paused);
        ctx.gl.queue_render();
        return true;
    }
    false
}

/// Optimistic icon swap so the click is felt immediately. The 1 Hz transport
/// tick will reconcile with mpv's `pause` + `core-idle` shortly after.
fn flip_play_icon(btn: &gtk::Button, now_paused: bool) {
    let (icon, tip) = if now_paused {
        ("media-playback-start-symbolic", "Play (Space)")
    } else {
        ("media-playback-pause-symbolic", "Pause (Space)")
    };
    if btn.icon_name().as_deref() != Some(icon) {
        btn.set_icon_name(icon);
    }
    btn.set_tooltip_text(Some(tip));
}

fn schedule_warm_reveal(ctx: PlayToggleCtx) {
    let _ = glib::timeout_add_local(Duration::from_millis(WARM_REVEAL_DELAY_MS), move || {
        ctx.recent.set_visible(false);
        (ctx.on_video_chrome)();
        schedule_window_fit_h_video(ctx.player.clone(), ctx.win.clone());
        if let Some(button) = ctx.sub_menu.as_ref() {
            schedule_sub_button_scan(ctx.player.clone(), button.clone());
        }
        ctx.win.present();
        if let Some(b) = ctx.player.borrow().as_ref() {
            let _ = b.mpv.set_property("pause", false);
        }
        ctx.gl.queue_render();
        (ctx.on_file_loaded)();
        glib::ControlFlow::Break
    });
}

fn wire_play_toggles(play_pause: &gtk::Button, ctx: PlayToggleCtx) {
    {
        let btn_ctx = ctx.clone();
        play_pause.connect_clicked(move |_| {
            toggle_play_pause(&btn_ctx);
        });
    }

    let rpp = gtk::GestureClick::new();
    rpp.set_button(gtk::gdk::BUTTON_SECONDARY);
    rpp.set_propagation_phase(gtk::PropagationPhase::Capture);
    let gl = ctx.gl.clone();
    {
        let press_ctx = ctx;
        rpp.connect_pressed(move |gest, n_press, _, _| {
            let _ = gest.set_state(gtk::EventSequenceState::Claimed);
            if n_press == 1 {
                toggle_play_pause(&press_ctx);
            }
        });
    }
    gl.add_controller(rpp);
}
