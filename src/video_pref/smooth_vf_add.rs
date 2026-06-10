// Smooth 60 `vf add` path: env prep + attach + failure recovery (reload once, else prefs off).

/// Publish env / resolve script before **`vf add`**. Returns `true` when Smooth was turned off in prefs.
fn prep_smooth_60_for_vf(
    mpv: &Mpv,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    cadence_hz: Option<f64>,
) -> bool {
    if !v.smooth_60 || !mpv_has_open_media(mpv) || !smooth_wants_vapoursynth_vf(mpv, bundle, speed_hint) {
        return false;
    }
    ensure_hwdec_vf_copy(mpv);
    match speed_hint {
        Some(s) => set_playback_speed_env(s),
        None => set_playback_speed_env_from_mpv(mpv),
    }
    let cap_px = effective_smooth_me_budget_px(mpv, v, bundle);
    let fps_opt = cadence_hz.or_else(|| refresh_smooth_cadence_gate(mpv, bundle));
    if v.vs_path.trim().is_empty() {
        publish_smooth_me_budget_env(cap_px);
    }
    apply_source_fps_env(fps_opt);
    let epoch = VPY_LOG_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::set_var(RHINO_VPY_LOG_EPOCH_VAR, format!("{epoch}"));
    if !apply_mvtools_env(v) {
        turn_off_smooth_60_in_prefs(v);
        return true;
    }
    if resolve_vs_script_path(v).is_none() {
        eprintln!(
            "[rhino] video: VapourSynth: no .vpy (install mvtools + data/vs bundle; see `data/vs/README.md`)."
        );
        turn_off_smooth_60_in_prefs(v);
        return true;
    }
    false
}

/// True when [add_smooth_60] should not even try **`vf add`** right now (no media, mid-load, busy, present).
fn add_smooth_60_blocked(
    mpv: &Mpv,
    speed_hint: Option<f64>,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> bool {
    if !mpv_has_open_media(mpv) {
        eprintln!(
            "[rhino] video: VapourSynth deferred (no `path` yet — will apply after loadfile)"
        );
        return true;
    }
    // `vf add` during a fresh load (resume seek not applied yet, decoder idle) fails with
    // MPV_ERROR_COMMAND and the retry storm can leave the vapoursynth wrapper unusable for the
    // rest of the process. The debounced transport resync self-retries until resume completes.
    if bundle.is_some_and(|b| b.resume_seek_pending()) {
        eprintln!("[rhino] video: VapourSynth deferred (resume seek pending — attach after it settles)");
        return true;
    }
    !smooth_wants_vapoursynth_vf(mpv, bundle, speed_hint)
        || bundle.is_some_and(|b| b.smooth_vf_attach_pending())
        || vf_chain_has_vapoursynth(mpv)
}

fn note_bundled_me_budget_applied_for_open(
    mpv: &Mpv,
    v: &VideoPrefs,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    me_cap: u64,
) {
    if !v.vs_path.trim().is_empty() {
        return;
    }
    let media_key = me_budget_local_path(mpv, bundle)
        .as_ref()
        .and_then(|p| crate::db::history_key(p.as_path()));
    note_bundled_me_budget_vf_applied(me_cap, media_key);
}

/// `vf add` failed and no filter is present: reload the open media once, else turn Smooth off.
/// Returns `true` when prefs were disabled.
fn recover_failed_vf_add(
    mpv: &Mpv,
    v: &mut VideoPrefs,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> bool {
    if let Some(b) = bundle {
        let resume_playing = !mpv.get_property::<bool>("pause").unwrap_or(true);
        if !b.smooth_vf_reload_attempted()
            && b.smooth_vf_stripped_this_open()
            && reload_open_media_for_vf_reset(b, resume_playing)
        {
            b.set_smooth_vf_reload_attempted(true);
            return false;
        }
    }
    turn_off_smooth_60_in_prefs(v);
    true
}

/// Attach the ~60 fps VapourSynth filter when [VideoPrefs::smooth_60]. Returns `true` if Smooth
/// was **disabled** in prefs (script missing or `vf add` failed beyond recovery).
fn add_smooth_60(
    mpv: &Mpv,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    cadence_hz: Option<f64>,
) -> bool {
    if !v.smooth_60 || add_smooth_60_blocked(mpv, speed_hint, bundle) {
        return false;
    }
    if prep_smooth_60_for_vf(mpv, v, speed_hint, bundle, cadence_hz) {
        return true;
    }
    let Some(p) = resolve_vs_script_path(v) else {
        return true;
    };
    eprintln!("[rhino] video: VapourSynth script = {p}");
    let p_esc = mpv_escape_path(&p);
    let me_cap = effective_smooth_me_budget_px(mpv, v, bundle);
    if !smooth_vapoursynth_vf_try_attach(mpv, &p_esc, bundle) {
        if vf_chain_has_vapoursynth(mpv) {
            eprintln!("[rhino] video: vapoursynth vf present after add error — keeping Smooth on");
            note_bundled_me_budget_applied_for_open(mpv, v, bundle, me_cap);
            apply_smooth_vf_present_opts(mpv);
            return false;
        }
        return recover_failed_vf_add(mpv, v, bundle);
    }
    note_bundled_me_budget_applied_for_open(mpv, v, bundle, me_cap);
    apply_smooth_vf_present_opts(mpv);
    false
}
