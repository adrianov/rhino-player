use crate::mpv_embed::MpvBundle;

fn post_smooth_60_state(mpv: &Mpv, v: &VideoPrefs, want_60: bool, disabled_60: bool, vlog: bool) {
    if want_60 && !v.smooth_60 {
        if let Err(e) = mpv.set_property("hwdec", "auto") {
            eprintln!("[rhino] video: set hwdec auto after VapourSynth off: {e:?}");
        } else {
            eprintln!("[rhino] video: hwdec=auto (VapourSynth path missing or vf rejected)");
        }
        let _ = mpv.set_property("vd-lavc-dr", "auto");
    }
    if disabled_60 {
        eprintln!("[rhino] video: saved `video_smooth_60` = 0 (VapourSynth path unusable or vf rejected).");
    }
    log_vf_diagnostics(mpv, vlog);
}

fn set_auto_decode(mpv: &Mpv, vlog: bool) {
    let need_hwdec = mpv
        .get_property::<String>("hwdec")
        .map(|s| s != "auto")
        .unwrap_or(true);
    if need_hwdec {
        if let Err(e) = mpv.set_property("hwdec", "auto") {
            eprintln!("[rhino] video: set hwdec auto failed: {e:?}");
        } else if vlog {
            eprintln!("[rhino] video: hwdec=auto (no mvtools vf: smooth off or speed ≠ 1.0×)");
        }
    }
    let need_dr = mpv
        .get_property::<String>("vd-lavc-dr")
        .map(|s| s != "auto")
        .unwrap_or(true);
    if need_dr {
        let _ = mpv.set_property("vd-lavc-dr", "auto");
    }
}

/// Plain playback after Smooth **off** / **`vf`** strip — prefer **`clear_vf`** which ends with this once **`vf`** is empty.
///
/// **Linux:** **`audio`** then swap gate off — never **`display-resample`** without **`report_swap`**.
/// **macOS:** **`display-resample`** + swap (**`CVDisplayLink`**); fallback **`audio`** + gate off on failure.
/// **`vf clr`** runs in **`with_macos_vf_teardown`** when a bundle is passed.
fn restore_non_smooth_present_opts(mpv: &Mpv) {
    let _ = mpv.set_property("interpolation", "no");
    #[cfg(target_os = "macos")]
    {
        if mpv.set_property("video-sync", "display-resample").is_ok() {
            smooth_vf_swap_timing_set(true);
        } else {
            let _ = mpv.set_property("video-sync", "audio");
            smooth_vf_swap_timing_set(false);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = mpv.set_property("video-sync", "audio");
        smooth_vf_swap_timing_set(false);
    }
}

/// FlowFPS outputs ~60 fps from the vf chain; **audio**-locked sync often collapses visible cadence.
/// Match presentation to the display timeline (`display-resample`) and disable shader **`interpolation`**.
/// **`hwdec`** / **`vd-lavc-dr`** stay whatever mpv already uses (typically auto).
fn apply_smooth_vf_present_opts(mpv: &Mpv) {
    if let Err(e) = mpv.set_property("video-sync", "display-resample") {
        if video_log() {
            eprintln!("[rhino] video: (verbose) video-sync display-resample failed: {e:?}");
        }
    }
    let _ = mpv.set_property("interpolation", "no");
    if video_log() {
        eprintln!(
            "[rhino] video: (verbose) smooth vf: video-sync=display-resample interpolation=no"
        );
    }
    // Enable swap reports last so **`report_swap`** cannot fire until **`display-resample`** is active.
    smooth_vf_swap_timing_set(true);
}

fn clear_vf(mpv: &Mpv, bundle: Option<&MpvBundle>, vlog: bool) {
    let inner = || {
        if let Err(e) = mpv.command("vf", &["clr", ""]) {
            eprintln!("[rhino] video: vf clr failed: {e:?}; trying set_property vf");
            if let Err(e2) = mpv.set_property("vf", "") {
                eprintln!("[rhino] video: set_property vf (clear) failed: {e2:?}");
            }
        } else if vlog {
            eprintln!("[rhino] video: vf clr ok");
        }
        let _ = mpv.set_property("vf", "");
        restore_non_smooth_present_opts(mpv);
    };
    #[cfg(target_os = "macos")]
    {
        if let Some(b) = bundle {
            b.with_macos_vf_teardown(inner);
            b.macos_ping_render_context();
            b.macos_mark_display_pending();
        } else {
            inner();
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle;
        inner();
    }
}

fn log_vf_diagnostics(mpv: &Mpv, vlog: bool) {
    match mpv.get_property::<String>("vf") {
        Ok(s) if !s.is_empty() => eprintln!("[rhino] video: mpv property `vf` = {s:?}"),
        Ok(_) => {
            eprintln!("[rhino] video: mpv property `vf` is empty (no file, or not applied yet)")
        }
        Err(e) => eprintln!("[rhino] video: could not read mpv property `vf`: {e:?}"),
    }
    if vlog {
        if let Ok(s) = mpv.get_property::<String>("video-sync") {
            eprintln!("[rhino] video: (verbose) video-sync = {s:?}");
        }
    }
}

/// If Smooth 60 is on, **speed** is ~1.0×, and `vapoursynth` is still not in the `vf` list, run
/// [apply_mpv_video] once (covers a rare missed attach).
pub fn reapply_60_if_still_missing(b: &MpvBundle, v: &mut VideoPrefs) -> MpvVideoApply {
    reapply_60_if_still_missing_impl(b, v, false)
}

/// Like [reapply_60_if_still_missing] but skips the mpv **`pause`** gate — use after transport
/// **`Pause(false)`** when **`get_property("pause")`** may still read **paused**.
pub fn reapply_60_after_transport_unpause(b: &MpvBundle, v: &mut VideoPrefs) -> MpvVideoApply {
    reapply_60_if_still_missing_impl(b, v, true)
}

fn reapply_60_if_still_missing_impl(
    b: &MpvBundle,
    v: &mut VideoPrefs,
    trust_playing_from_transport: bool,
) -> MpvVideoApply {
    let mpv = &b.mpv;
    if !trust_playing_from_transport && mpv.get_property::<bool>("pause").unwrap_or(true) {
        return MpvVideoApply::default();
    }
    if !v.smooth_60 || !mpv_has_open_media(mpv) {
        return MpvVideoApply::default();
    }
    if !mvtools_vf_eligible(mpv, None) {
        return MpvVideoApply::default();
    }
    if vf_string_has_vapoursynth(mpv) {
        return MpvVideoApply::default();
    }
    eprintln!("[rhino] video: reapply_60_if_still_missing → apply_mpv_video");
    if trust_playing_from_transport {
        apply_mpv_video_after_transport_unpause(b, v, None)
    } else {
        apply_mpv_video(b, v, None)
    }
}

pub(crate) fn vf_chain_has_vapoursynth(mpv: &Mpv) -> bool {
    vf_string_has_vapoursynth(mpv)
}

fn vf_string_has_vapoursynth(mpv: &Mpv) -> bool {
    match mpv.get_property::<String>("vf") {
        Ok(s) => s.to_lowercase().contains("vapoursynth"),
        Err(_) => false,
    }
}

/// Drop the vapoursynth `vf` immediately before a **seek** (or similar position jump) when it is
/// still present so mpv can decode a real frame — especially while **paused**. Plain pause/unpause
/// does not call this.
pub fn unload_smooth_on_pause(mpv: &Mpv) -> bool {
    if !vf_string_has_vapoursynth(mpv) {
        return false;
    }
    let vlog = video_log();
    clear_vf(mpv, None, vlog);
    set_auto_decode(mpv, vlog);
    true
}
pub fn apply_mpv_video_init(mpv: &Mpv, v: &mut VideoPrefs) -> MpvVideoApply {
    apply_mpv_video_impl(mpv, None, v, None, None)
}

/// Normal playback is intentionally a no-op: leave mpv's timing, decode, and filter defaults alone.
/// When Smooth 60 is active, replace the `vf` list and add VapourSynth at ~**1.0×** only.
/// [speed_hint] is passed to [add_smooth_60] when set (e.g. header row) to match env before the [vf] add.
fn log_apply(v: &VideoPrefs) {
    eprintln!(
        "[rhino] video: apply_mpv_video smooth_60={} vs_path_len={}",
        v.smooth_60,
        v.vs_path.len()
    );
    if !v.smooth_60 {
        eprintln!(
            "[rhino] video: smooth_60 off — no 60 fps vf. Enable **Preferences** → **Smooth Video (60 FPS)** for VapourSynth (bundled .vpy if path is empty)."
        );
    }
}

pub fn apply_mpv_video(b: &MpvBundle, v: &mut VideoPrefs, speed_hint: Option<f64>) -> MpvVideoApply {
    apply_mpv_video_impl(&b.mpv, Some(b), v, speed_hint, None)
}

/// Runs [apply_mpv_video_impl] with **`pause=no`** assumed for MVTools eligibility — mpv's **`pause`**
/// property can lag **`observe_property(`pause`)`** right after unpause, so the normal path would skip
/// attaching Smooth even though playback has resumed.
pub fn apply_mpv_video_after_transport_unpause(
    b: &MpvBundle,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
) -> MpvVideoApply {
    apply_mpv_video_impl(&b.mpv, Some(b), v, speed_hint, Some(false))
}

fn apply_mpv_video_impl(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    pause_override: Option<bool>,
) -> MpvVideoApply {
    let vlog = video_log();
    log_apply(v);
    let paused =
        pause_override.unwrap_or_else(|| mpv.get_property::<bool>("pause").unwrap_or(true));
    let use_mvtools = v.smooth_60 && mvtools_vf_eligible(mpv, speed_hint) && !paused;
    let want_60 = v.smooth_60;
    let had_vapoursynth = vf_string_has_vapoursynth(mpv);
    if !use_mvtools {
        let keep_vf_during_pause = paused && want_60;
        let stripped_vf = had_vapoursynth && !keep_vf_during_pause;
        if stripped_vf {
            clear_vf(mpv, bundle, vlog);
            set_auto_decode(mpv, vlog);
            if !want_60 {
                smooth_off_refresh_playhead(mpv, bundle);
            }
        }
        if !want_60 && !stripped_vf {
            restore_non_smooth_present_opts(mpv);
        }
        post_smooth_60_state(mpv, v, want_60, false, vlog);
        return MpvVideoApply::default();
    }
    if !mpv_has_open_media(mpv) {
        let disabled_60 = add_smooth_60(mpv, v, speed_hint);
        post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
        return MpvVideoApply {
            smooth_auto_off: disabled_60,
        };
    }

    if had_vapoursynth && vf_smooth_matches_prefs(mpv, v) {
        match speed_hint {
            Some(s) => set_playback_speed_env(s),
            None => set_playback_speed_env_from_mpv(mpv),
        }
        let fps_env_before = std::env::var(crate::paths::RHINO_SOURCE_FPS_VAR).ok();
        set_source_fps_env_from_mpv(mpv);
        let fps_env_after = std::env::var(crate::paths::RHINO_SOURCE_FPS_VAR).ok();
        // `RHINO_SOURCE_FPS` is read when the `.vpy` graph starts; refreshing env alone does not
        // re-run the script after `vf add`. Rebuild when cadence becomes known or changes (e.g.
        // `container-fps` lagged behind the first attach).
        if fps_env_before == fps_env_after {
            apply_smooth_vf_present_opts(mpv);
            post_smooth_60_state(mpv, v, want_60, false, vlog);
            return MpvVideoApply::default();
        }
        eprintln!(
            "[rhino] video: rebuilding vapoursynth vf ({} changed)",
            crate::paths::RHINO_SOURCE_FPS_VAR
        );
        clear_vf(mpv, bundle, vlog);
        let disabled_60 = add_smooth_60(mpv, v, speed_hint);
        post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
        return MpvVideoApply {
            smooth_auto_off: disabled_60,
        };
    }

    // Smooth vf presentation + swap timing; stripping vf restores plain opts (clear_vf).
    clear_vf(mpv, bundle, vlog);
    let disabled_60 = add_smooth_60(mpv, v, speed_hint);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    MpvVideoApply {
        smooth_auto_off: disabled_60,
    }
}
