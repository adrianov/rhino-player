/// Load a file, hide the recent grid overlay, show video; [LoadOpts::record] appends to recent history.
/// [play_on_start]: clear `pause` so playback runs after the SQLite resume `start=` is applied.
/// **false** for CLI open-on-launch to respect saved state.
fn try_load(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
) -> Result<(), String> {
    let raw = path.to_path_buf();
    let path = crate::video_ext::resolve_open_media_path(path);
    if path != raw {
        eprintln!(
            "[rhino] resolve_open: {} -> {}",
            raw.display(),
            path.display()
        );
    }
    let tag = if o.warm_preload { "warm_preload" } else { "try_load" };
    eprintln!(
        "[rhino] {tag}: path={} exists={} record={} player_ready={} play={}",
        path.display(),
        path.exists(),
        o.record,
        player.borrow().is_some(),
        o.play_on_start
    );
    if o.play_on_start && !o.warm_preload {
        crate::app::cancel_warm_preload_for_playback();
        if let Some(pf) = o.playback_focus.as_ref() {
            pf.set(true);
        }
    }
    let warm_hit = load_file_into_player(&path, player, recent_layer, o)?;
    *o.last_path.borrow_mut() = std::fs::canonicalize(&path).ok();
    if o.record {
        history::record(&path);
    }
    let ttl = title_for_open_path(&path);
    sync_app_window_title(win, o.hdr_title_mirror.as_deref(), Some(ttl.as_str()));
    // Drain `FileLoaded` / `path` before `reveal_ui_after_load` unpause so transport runs
    // `forget_bundled_me_budget_vf_apply_on_new_media` and resume/audio restore before `Pause(false)`
    // can attach Smooth (`note_bundled` was being cleared by a later `FileLoaded` → duplicate `vf add`).
    transport_drain_after_loadfile();
    reveal_ui_after_load(player, win, gl, recent_layer, o, warm_hit);
    let _ = glib::idle_add_local_once(transport_drain_after_loadfile);
    if let Some(f) = o.on_loaded.clone() {
        glib::source::idle_add_local_once(move || f());
    }
    Ok(())
}

/// Calls `loadfile` on the player, or detects a warm preload hit.
/// Returns `true` if the file was already loaded (warm hit).
fn load_file_into_player(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
) -> Result<bool, String> {
    let mut g = player.borrow_mut();
    let b = g.as_mut().ok_or("Player not ready. Wait for GL init.")?;
    let prev = crate::media_probe::shell_media_path(
        &b.mpv,
        b.me_budget_shell_path.borrow().as_deref(),
    )
    .or_else(|| o.last_path.borrow().clone());
    // Warm hit only for continue-grid hover / first-card preload — explicit card open must
    // reload so SQLite entity-global resume is applied (see `load_file_path`).
    if o.warm_preload
        && recent_layer.is_visible()
        && crate::media_probe::mpv_warm_hit_ready(&b.mpv, path)
    {
        if prev.as_ref().is_some_and(|p| !same_open_target(p, path)) {
            video_pref::strip_vapoursynth_before_replace_media(b);
            crate::seek_bar_preview::reset_on_main_media_change_from("try_load:warm_entity_change");
        }
        eprintln!("[rhino] warm_preload: warm hit (same file)");
        b.set_me_budget_shell_path(path);
        crate::video_pref::publish_smooth_env_before_load(path, &o.video_pref.borrow(), false);
        if o.play_on_start {
            b.set_skip_media_persist(false);
        }
        let _ = b.ensure_resume_before_unpause();
        if !o.play_on_start {
            let _ = b.mpv.set_property("pause", true);
        }
        transport_nudge_tick();
        return Ok(true);
    }
    if prev.as_ref().is_some_and(|p| !same_open_target(p, path)) {
        video_pref::strip_vapoursynth_before_replace_media(b);
        eprintln!(
            "[rhino] try_load: entity change {} -> {}",
            prev.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "?".into()),
            path.display()
        );
        crate::seek_bar_preview::reset_on_main_media_change_from("try_load:entity_change");
    }
    b.set_me_budget_shell_path(path);
    crate::video_pref::publish_smooth_env_before_load(path, &o.video_pref.borrow(), true);
    // Normalize speed before `loadfile` for sibling auto-advance (mpv keeps `speed`
    // across loadfile within a session; resume position is read from SQLite, not mpv).
    if o.reset_speed_to_normal {
        crate::playback_speed::force_normal(&b.mpv);
    }
    // Only EOF / last ~3s ([is_natural_end]): the old "mostly watched" (~85%) heuristic could drop
    // the previous continue entry while switching files if duration/`time-pos` was misleading.
    let clear_resume = {
        let outgoing = crate::media_probe::shell_media_path(
            &b.mpv,
            b.me_budget_shell_path.borrow().as_deref(),
        );
        is_natural_end(&b.mpv)
            && outgoing
                .as_ref()
                .is_some_and(|p| crate::sibling_advance::next_after_eof(p).is_none())
    };
    let drop_prev = prev.as_ref().is_some_and(|p| {
        !same_open_target(p, path) && is_natural_end(&b.mpv)
    });
    let snapshot_outgoing = !o.warm_preload;
    b.set_skip_media_persist(recent_layer.is_visible() && o.warm_preload);
    let tag = if o.warm_preload { "warm_preload" } else { "try_load" };
    let load_t0 = std::time::Instant::now();
    if let Err(e) = b.load_file_path(path, clear_resume, snapshot_outgoing, o.warm_preload, None) {
        eprintln!("[rhino] {tag}: loadfile failed: {e}");
        return Err(e);
    }
    eprintln!(
        "[rhino] {tag}: loadfile ok ms={}",
        load_t0.elapsed().as_millis()
    );
    if drop_prev && !o.warm_preload {
        if let Some(p) = prev {
            remove_continue_entry(&p);
        }
    }
    Ok(false)
}

/// Hides recent grid and kicks off playback (immediate or delayed warm reveal).
/// Always raises the window so openings from external handlers (e.g. file manager while in background)
/// bring the UI to the foreground.
fn reveal_ui_after_load(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
    warm_hit: bool,
) {
    win.present();
    let delayed_warm = warm_hit && o.play_on_start;
    if !delayed_warm {
        recent_layer.set_visible(false);
        if let Some(pf) = o.playback_focus.as_ref() {
            pf.set(true);
        }
        if let Some(f) = o.on_start.as_ref() {
            f();
        }
        #[cfg(target_os = "macos")]
        {
            crate::app::refresh_registered_shell_compositing();
            crate::macos_window::nudge_gdk_compositing_width(win);
            if let Some(b) = player.borrow().as_ref() {
                b.nudge_shell_layout_after_resize(gl);
            }
        }
    }
    gl.queue_render();
    if o.play_on_start {
        start_playback(player, win, gl, recent_layer, o, delayed_warm);
    }
    if let Some(b) = player.borrow().as_ref() {
        sync_window_aspect_from_mpv(&b.mpv, o.win_aspect.as_ref());
    }
    schedule_window_fit_h_video(Rc::clone(player), win.clone(), gl.clone());
}

/// Unpauses mpv; for warm-hit paths, delays reveal by [WARM_REVEAL_DELAY_MS].
fn start_playback(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
    delayed_warm: bool,
) {
    if let Some(b) = player.borrow().as_ref() {
        b.set_skip_media_persist(false);
    }
    if delayed_warm {
        let recent = recent_layer.as_ref().clone();
        let win2 = win.clone();
        let gl2 = gl.clone();
        let player2 = player.clone();
        let on_start = o.on_start.clone();
        let playback_focus = o.playback_focus.clone();
        let _ = glib::timeout_add_local(Duration::from_millis(WARM_REVEAL_DELAY_MS), move || {
            recent.set_visible(false);
            if let Some(pf) = playback_focus.as_ref() {
                pf.set(true);
            }
            if let Some(f) = on_start.as_ref() { f(); }
            win2.present();
            unpause_and_finish_resume(&player2);
            gl2.queue_render();
            glib::ControlFlow::Break
        });
    } else {
        win.present();
        unpause_and_finish_resume(player);
        gl.queue_render();
    }
}

fn save_mpv_audio(mpv: &Mpv) {
    let vol = mpv.get_property::<f64>("volume").unwrap_or(100.0);
    let muted = mpv.get_property::<bool>("mute").unwrap_or(false);
    db::save_audio(vol, muted);
}

fn save_mpv_state(mpv: &Mpv, sub: &RefCell<db::SubPrefs>) {
    save_mpv_audio(mpv);
    let mut p = sub.borrow_mut();
    if let Ok(sc) = mpv.get_property::<f64>("sub-scale") {
        if sc.is_finite() {
            p.scale = sc;
        }
    }
    db::save_sub(&p);
}

fn vol_icon(muted: bool, vol: f64) -> &'static str {
    if muted || vol < 0.5 {
        "audio-volume-muted-symbolic"
    } else if vol < 33.0 {
        "audio-volume-low-symbolic"
    } else if vol < 66.0 {
        "audio-volume-medium-symbolic"
    } else {
        "audio-volume-high-symbolic"
    }
}

/// Header sound popover: mute icon only (fader next to it shows level).
fn vol_mute_pop_icon(muted: bool) -> &'static str {
    if muted {
        "audio-volume-muted-symbolic"
    } else {
        "audio-volume-high-symbolic"
    }
}

include!("sibling_eof_state.rs");
