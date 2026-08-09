// loadfile / warm-hit path for try_load.

/// Calls `loadfile` on the player, or detects a warm preload hit.
/// Returns `true` if the file was already loaded (warm hit).
fn load_file_into_player(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
) -> Result<bool, String> {
    let mut g = player
        .try_borrow_mut()
        .map_err(|_| "Player busy (transport or load in progress).".to_string())?;
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
    // Clear Continue when the outgoing title is finished ([is_continue_done]): EOF / last seconds,
    // or past the watched threshold (Next during credits). Warm preload never clears.
    let finished = is_continue_done(&b.mpv);
    let clear_resume = finished
        && prev
            .as_ref()
            .is_some_and(|p| crate::sibling_advance::next_after_eof(p).is_none());
    let drop_prev =
        finished && prev.as_ref().is_some_and(|p| !same_open_target(p, path));
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
            eprintln!("[rhino] continue: drop finished {}", p.display());
            remove_continue_entry(&p);
        }
    }
    Ok(false)
}

