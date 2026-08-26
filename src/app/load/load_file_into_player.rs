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
    let (warm_hit, prev) = attempt_warm_hit(path, b, recent_layer, o);
    if warm_hit {
        return Ok(true);
    }
    prep_cold_load(b, path, prev.as_deref(), o);
    cold_load_into_player(b, path, prev.as_deref(), recent_layer, o)?;
    Ok(false)
}

/// Resolve the outgoing media target and serve a warm preload hit when eligible.
fn attempt_warm_hit(
    path: &Path,
    b: &mut MpvBundle,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
) -> (bool, Option<PathBuf>) {
    let prev = outgoing_media_target(b, o);
    let hit = load_as_warm_hit(path, b, prev.as_deref(), recent_layer, o);
    (hit, prev)
}

/// Entity-change reset, shell-path swap, smooth env publish before the `loadfile`.
fn prep_cold_load(b: &mut MpvBundle, path: &Path, prev: Option<&Path>, o: &LoadOpts) {
    reset_vf_for_entity_change(b, prev, path);
    b.set_me_budget_shell_path(path);
    crate::video_pref::publish_smooth_env_before_load(path, &o.video_pref.borrow(), true);
}

/// Path of the currently loaded media (shell-aware), falling back to the last open target.
fn outgoing_media_target(b: &MpvBundle, o: &LoadOpts) -> Option<PathBuf> {
    crate::media_probe::shell_media_path(&b.mpv, b.me_budget_shell_path.borrow().as_deref())
        .or_else(|| o.last_path.borrow().clone())
}

/// Warm hit only for continue-grid hover / first-card preload — explicit card open must
/// reload so SQLite entity-global resume is applied (see `load_file_path`).
/// Returns `true` when the warm hit was served (no `loadfile` needed).
fn load_as_warm_hit(
    path: &Path,
    b: &mut MpvBundle,
    prev: Option<&Path>,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
) -> bool {
    if !(o.warm_preload
        && recent_layer.is_visible()
        && crate::media_probe::mpv_warm_hit_ready(&b.mpv, path))
    {
        return false;
    }
    if prev.is_some_and(|p| !same_open_target(p, path)) {
        video_pref::strip_vapoursynth_before_replace_media(b);
        crate::seek_bar_preview::reset_on_main_media_change_from("try_load:warm_entity_change");
    }
    eprintln!("[rhino] warm_preload: warm hit (same file)");
    finish_warm_hit(path, b, o);
    transport_nudge_tick();
    true
}

/// Finish a warm hit: republish smooth env, arm resume, keep paused unless play-on-start.
fn finish_warm_hit(path: &Path, b: &mut MpvBundle, o: &LoadOpts) {
    b.set_me_budget_shell_path(path);
    crate::video_pref::publish_smooth_env_before_load(path, &o.video_pref.borrow(), false);
    if o.play_on_start {
        b.set_skip_media_persist(false);
    }
    let _ = b.ensure_resume_before_unpause();
    if !o.play_on_start {
        let _ = b.mpv.set_property("pause", true);
    }
}

/// Strip the outgoing clip's VapourSynth graph and reset preview state across an entity change.
fn reset_vf_for_entity_change(b: &mut MpvBundle, prev: Option<&Path>, path: &Path) {
    if !prev.is_some_and(|p| !same_open_target(p, path)) {
        return;
    }
    video_pref::strip_vapoursynth_before_replace_media(b);
    eprintln!(
        "[rhino] try_load: entity change {} -> {}",
        prev.map(|p| p.display().to_string())
            .unwrap_or_else(|| "?".into()),
        path.display()
    );
    crate::seek_bar_preview::reset_on_main_media_change_from("try_load:entity_change");
}

/// Cold-load path: normalize speed, decide resume clearing, run `loadfile`, then drop a finished
/// predecessor from the continue list.
fn cold_load_into_player(
    b: &mut MpvBundle,
    path: &Path,
    prev: Option<&Path>,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
) -> Result<(), String> {
    // Normalize speed before `loadfile` for sibling auto-advance (mpv keeps `speed`
    // across loadfile within a session; resume position is read from SQLite, not mpv).
    if o.reset_speed_to_normal {
        crate::playback_speed::force_normal(&b.mpv);
    }
    // Clear Continue when the outgoing title is finished ([is_continue_done]): EOF / last seconds,
    // or past the watched threshold (Next during credits). Warm preload never clears.
    let finished = is_continue_done(&b.mpv);
    let clear_resume =
        finished && prev.is_some_and(|p| crate::sibling_advance::next_after_eof(p).is_none());
    let drop_prev = finished && prev.is_some_and(|p| !same_open_target(p, path));
    b.set_skip_media_persist(recent_layer.is_visible() && o.warm_preload);
    run_cold_loadfile(b, path, clear_resume, !o.warm_preload, o.warm_preload)?;
    drop_finished_prev(drop_prev, prev, o);
    Ok(())
}

/// Run `loadfile` and log its duration under the caller's tag.
fn run_cold_loadfile(
    b: &mut MpvBundle,
    path: &Path,
    clear_resume: bool,
    snapshot_outgoing: bool,
    warm_preload: bool,
) -> Result<(), String> {
    let tag = if warm_preload {
        "warm_preload"
    } else {
        "try_load"
    };
    let load_t0 = std::time::Instant::now();
    if let Err(e) = b.load_file_path(path, clear_resume, snapshot_outgoing, warm_preload, None) {
        eprintln!("[rhino] {tag}: loadfile failed: {e}");
        return Err(e);
    }
    eprintln!(
        "[rhino] {tag}: loadfile ok ms={}",
        load_t0.elapsed().as_millis()
    );
    Ok(())
}

/// Remove a finished predecessor from the continue list after switching away from it (never on
/// warm preload).
fn drop_finished_prev(drop_prev: bool, prev: Option<&Path>, o: &LoadOpts) {
    if !drop_prev || o.warm_preload {
        return;
    }
    if let Some(p) = prev {
        eprintln!("[rhino] continue: drop finished {}", p.display());
        remove_continue_entry(p);
    }
}
