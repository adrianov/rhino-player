#[derive(Clone)]
struct PlayToggleCtx {
    app: adw::Application,
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    win: adw::ApplicationWindow,
    video_handle: gtk::WindowHandle,
    gl: gtk::GLArea,
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    win_aspect: Rc<WinAspectCell>,
    sub_menu: Option<gtk::MenuButton>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
    /// Bottom-bar play/pause button. The toggle handler updates its icon
    /// optimistically so the click feels instant; the 1 Hz transport tick
    /// reconciles with mpv's actual state right after.
    play_pause: gtk::Button,
    /// Shared with [SiblingEofState]: resume a paused incomplete download on unpause.
    incomplete_hold: Rc<crate::incomplete_download_eof::IncompleteEofHold>,
}

/// Pause or unpause through mpv (bottom bar play control and Linux MPRIS). Returns [`false`] when no
/// media is loaded, the welcome grid covers video, or the engine rejects the pause write.
///
/// Matches vapoursynth smooth unpause bookkeeping used by the spacer / play button wiring. Uses a
/// single [`RefCell`] borrow for mpv reads and the pause write so state stays consistent on the GTK
/// main thread (callbacks do not run concurrently).
fn apply_mpv_pause(ctx: &PlayToggleCtx, want_pause: bool) -> bool {
    if ctx.recent.is_visible() {
        return false;
    }
    let g = ctx.player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    if b.mpv.get_property::<f64>("duration").unwrap_or(0.0) <= 0.0 {
        return false;
    }
    let cur_pause = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    if cur_pause == want_pause {
        return true;
    }

    let unpausing_into_play = !want_pause && cur_pause;
    let smooth_off = if unpausing_into_play {
        resync_smooth_on_unpause(ctx)
    } else {
        false
    };
    if smooth_off {
        sync_smooth_60_to_off(&ctx.app);
    }
    if unpausing_into_play {
        prep_incomplete_hold_on_unpause(ctx, b);
    }

    commit_pause_write(ctx, b, want_pause)
}

/// Write the pause flag; optimistic icon swap + repaint on success.
fn commit_pause_write(ctx: &PlayToggleCtx, b: &MpvBundle, want_pause: bool) -> bool {
    if b.mpv.set_property("pause", want_pause).is_ok() {
        flip_play_icon(&ctx.play_pause, want_pause);
        ctx.gl.queue_render();
        true
    } else {
        false
    }
}

/// Speed-mismatch Smooth resync bookkeeping on unpause; returns the auto-off flag.
fn resync_smooth_on_unpause(ctx: &PlayToggleCtx) -> bool {
    let mut pref = ctx.video_pref.borrow_mut();
    video_pref::resync_smooth_if_speed_mismatch(&ctx.player, &mut pref).smooth_auto_off
}

/// Resume a paused incomplete download when unpausing into play (shared with [SiblingEofState]).
fn prep_incomplete_hold_on_unpause(ctx: &PlayToggleCtx, b: &MpvBundle) {
    if let Some(p) = local_file_from_mpv(&b.mpv)
        .or_else(|| ctx.last_path.borrow().clone())
        .as_deref()
    {
        ctx.incomplete_hold.on_unpause(&b.mpv, p);
    }
}

/// Stop-style pause (media keys **Stop** shell action + MPRIS `Stop`): hold position, show play icon.
fn media_stop(play_key: &PlayToggleCtx) {
    let _ = apply_mpv_pause(play_key, true);
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
        let opened = open_from_warm_grid(ctx, b);
        drop(g);
        if opened {
            schedule_warm_reveal(ctx.clone());
        }
        return opened;
    }
    let paused = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    drop(g);
    crate::user_action_log::act(format!(
        "play/pause button -> {}",
        if paused { "play" } else { "pause" }
    ));
    apply_mpv_pause(ctx, !paused)
}

/// Grid-covered tap acts as an explicit open from the warm card: refresh title/aspect chrome and
/// report success so the caller schedules the delayed reveal.
fn open_from_warm_grid(ctx: &PlayToggleCtx, b: &MpvBundle) -> bool {
    crate::user_action_log::act("play/pause (continue grid) -> open from warm card");
    if let Some(path) = local_file_from_mpv(&b.mpv) {
        *ctx.last_path.borrow_mut() = std::fs::canonicalize(&path).ok();
        sync_app_window_title(
            &ctx.win,
            ctx.hdr_title_mirror.as_deref(),
            Some(title_for_open_path(&path).as_str()),
        );
    }
    sync_window_aspect_from_mpv(&b.mpv, ctx.win_aspect.as_ref());
    ctx.gl.queue_render();
    true
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
    crate::app::cancel_warm_preload_for_playback();
    ctx.playback_focus.set(true);
    // Strip stays for the warm beat; drop search IM immediately so the badge cannot linger.
    crate::recent_view::dismiss_search_for_playback();
    let _ = glib::timeout_add_local(Duration::from_millis(WARM_REVEAL_DELAY_MS), move || {
        run_warm_reveal_step(&ctx);
        glib::ControlFlow::Break
    });
}

/// One delayed-reveal tick: hide the grid, restore video chrome, present, unpause + resume.
fn run_warm_reveal_step(ctx: &PlayToggleCtx) {
    crate::recent_view::hide_continue_strip(&ctx.recent);
    (ctx.on_video_chrome)();
    schedule_window_fit_h_video(ctx.player.clone(), ctx.win.clone(), ctx.gl.clone());
    if let Some(button) = ctx.sub_menu.as_ref() {
        sync_sub_button_after_load(ctx.player.clone(), button.clone());
    }
    ctx.win.present();
    unpause_and_finish_resume(&ctx.player);
    ctx.gl.queue_render();
    (ctx.on_file_loaded)();
}

fn wire_play_toggles(play_pause: &gtk::Button, ctx: PlayToggleCtx) {
    {
        let btn_ctx = ctx.clone();
        play_pause.connect_clicked(move |_| {
            toggle_play_pause(&btn_ctx);
        });
    }

    // Secondary click on WindowHandle: use GestureClick + Claimed so GTK’s pointer/active-state
    // bookkeeping stays paired (EventControllerLegacy + Stop risked “broken accounting” warnings).
    let sec = gtk::GestureClick::new();
    sec.set_button(gtk::gdk::BUTTON_SECONDARY);
    sec.set_propagation_phase(gtk::PropagationPhase::Capture);
    let vh_ctx = ctx.clone();
    sec.connect_pressed(move |gesture, _n_press, _x, _y| {
        if toggle_play_pause(&vh_ctx) {
            let _ = gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    ctx.video_handle.add_controller(sec);
}
