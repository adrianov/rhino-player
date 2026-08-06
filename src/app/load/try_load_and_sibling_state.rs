/// Load a file, hide the recent grid overlay, show video; [LoadOpts::record] appends to recent history.
/// [play_on_start]: clear `pause` so playback runs after the SQLite resume `start=` is applied.
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
    if !o.warm_preload {
        if let Some(msg) = crate::media_open_fail::preflight_user_message(&path) {
            return fail_open(o, &path, msg.to_string());
        }
    }
    if o.play_on_start && !o.warm_preload {
        crate::app::cancel_warm_preload_for_playback();
        if let Some(pf) = o.playback_focus.as_ref() {
            pf.set(true);
        }
    }
    let warm_hit = match load_file_into_player(&path, player, recent_layer, o) {
        Ok(v) => v,
        Err(e) => return fail_open(o, &path, e),
    };
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
    // FileLoaded can run while the continue grid is still visible (drain precedes reveal), and
    // unpause may race a live player borrow — attach Smooth on the next idle once reveal settles.
    if o.play_on_start && o.video_pref.borrow().smooth_60 {
        glib::idle_add_local_once(request_smooth_60_transport_resync);
    }
    if let Some(f) = o.on_loaded.clone() {
        glib::source::idle_add_local_once(move || f());
    }
    Ok(())
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


fn fail_open(o: &LoadOpts, path: &Path, err: String) -> Result<(), String> {
    let msg = crate::media_open_fail::message_for_load_err(&err, path);
    eprintln!("[rhino] open failed: {msg} ({})", path.display());
    // User-initiated open (toast callback present): drop hollow/corrupt continue cards.
    // Warm preload leaves `on_open_fail` unset and must not rewrite history.
    if o.on_open_fail.is_some() && crate::media_open_fail::should_drop_from_continue(&msg) {
        remove_continue_entry(path);
    }
    if let Some(f) = o.on_open_fail.as_ref() {
        f(msg.clone());
    }
    Err(msg)
}

include!("sibling_eof_state.rs");
