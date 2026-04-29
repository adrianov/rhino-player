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

fn set_smooth_decode(mpv: &Mpv, vlog: bool) {
    if let Err(e) = mpv.set_property("hwdec", "no") {
        eprintln!("[rhino] video: set hwdec no failed: {e:?}");
    } else {
        eprintln!("[rhino] video: hwdec=no (vapoursynth vf: software decode so the filter path runs; hwdec=auto often skips it — see docs/features/26-sixty-fps-motion.md)");
    }
    if let Err(e) = mpv.set_property("vd-lavc-dr", "no") {
        eprintln!("[rhino] video: set vd-lavc-dr no failed: {e:?}");
    } else if vlog {
        eprintln!("[rhino] video: vd-lavc-dr=no (with smooth 60 at 1.0×)");
    }
}

fn set_auto_decode(mpv: &Mpv, vlog: bool) {
    if let Err(e) = mpv.set_property("hwdec", "auto") {
        eprintln!("[rhino] video: set hwdec auto failed: {e:?}");
    } else if vlog {
        eprintln!("[rhino] video: hwdec=auto (no mvtools vf: smooth off or speed ≠ 1.0×)");
    }
    let _ = mpv.set_property("vd-lavc-dr", "auto");
}

fn clear_vf(mpv: &Mpv, vlog: bool) {
    if let Err(e) = mpv.command("vf", &["clr", ""]) {
        eprintln!("[rhino] video: vf clr failed: {e:?}; trying set_property vf");
        if let Err(e2) = mpv.set_property("vf", "") {
            eprintln!("[rhino] video: set_property vf (clear) failed: {e2:?}");
        }
    } else if vlog {
        eprintln!("[rhino] video: vf clr ok");
    }
    let _ = mpv.set_property("vf", "");
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

/// After [loadfile]: draw **without** the VapourSynth `vf`, but already on the **Smooth** decode path
/// (`hwdec=no`, `vd-lavc-dr=no`) so the next idle only runs `vf add` instead of swapping hw→sw decode
/// and attaching the script at once — that combined jump commonly produced several seconds of black.
/// Attach VapourSynth on the following idle ([apply_mpv_video]). If Smooth 60 is off, not ~1.0×, or no media
/// is open yet, this delegates to [apply_mpv_video].
pub fn apply_mpv_fast_start_after_load(mpv: &Mpv, v: &mut VideoPrefs) -> MpvVideoApply {
    let use_mvtools_now = v.smooth_60 && mvtools_vf_eligible(mpv, None);
    if !use_mvtools_now || !mpv_has_open_media(mpv) {
        return apply_mpv_video(mpv, v, None);
    }
    let vlog = video_log();
    if vf_string_has_vapoursynth(mpv) {
        clear_vf(mpv, vlog);
    }
    set_smooth_decode(mpv, vlog);
    if video_log() {
        eprintln!("[rhino] video: after load — sw decode without vf yet; Smooth `vf` on next idle")
    }
    log_vf_diagnostics(mpv, vlog);
    MpvVideoApply {
        smooth_auto_off: false,
    }
}

/// [apply_mpv_video] when the VapourSynth [vf] was not installed yet; see [mvtools_vf_eligible] for when
/// the filter is actually added.
pub fn complete_vapoursynth_attach(mpv: &Mpv, v: &mut VideoPrefs) -> bool {
    eprintln!("[rhino] video: complete_vapoursynth_attach");
    apply_mpv_video(mpv, v, None).smooth_auto_off
}

/// If Smooth 60 is on, **speed** is ~1.0×, and `vapoursynth` is still not in the `vf` list (e.g. post-load
/// race), run [apply_mpv_video] once. Called from the **second** [loadfile] idle (chained), not from a timer.
pub fn reapply_60_if_still_missing(mpv: &Mpv, v: &mut VideoPrefs) -> bool {
    if mpv.get_property::<bool>("pause").unwrap_or(true) {
        return false;
    }
    if !v.smooth_60 || !mpv_has_open_media(mpv) {
        return false;
    }
    if !mvtools_vf_eligible(mpv, None) {
        return false;
    }
    if vf_string_has_vapoursynth(mpv) {
        return false;
    }
    complete_vapoursynth_attach(mpv, v)
}

fn vf_string_has_vapoursynth(mpv: &Mpv) -> bool {
    match mpv.get_property::<String>("vf") {
        Ok(s) => s.to_lowercase().contains("vapoursynth"),
        Err(_) => false,
    }
}

/// Drop the Smooth / VapourSynth `vf` while **paused** so the player shows a normal still frame;
/// restore with [apply_mpv_video] when playback resumes (transport `pause` handling).
pub fn unload_smooth_on_pause(mpv: &Mpv) -> bool {
    if !vf_string_has_vapoursynth(mpv) {
        return false;
    }
    let vlog = video_log();
    clear_vf(mpv, vlog);
    set_auto_decode(mpv, vlog);
    true
}

/// After the video filter list or decode path changes, re-align the video track to the audio clock
/// by [seek]ing to the current position (libmpv, same as input.conf). Skips at file start to avoid
/// fighting [try_load], and with zero/invalid duration.
fn resync_av_after_vf_change(mpv: &Mpv) {
    if !mpv_has_open_media(mpv) {
        return;
    }
    let dur = mpv.get_property::<f64>("duration").unwrap_or(0.0);
    if !dur.is_finite() || dur <= 0.0 {
        return;
    }
    let pos = match mpv.get_property::<f64>("time-pos") {
        Ok(p) if p.is_finite() && p >= 0.0 => p,
        _ => return,
    };
    if pos < 0.12 {
        return;
    }
    let end = (dur - 0.05).max(0.0);
    let t = pos.clamp(0.0, end);
    let s = format!("{:.4}", t);
    match mpv.command("seek", &[s.as_str(), "absolute+exact"]) {
        Ok(()) => {
            if video_log() {
                eprintln!("[rhino] video: A/V resync after vf (seek) @ {t:.3}s");
            }
        }
        Err(e) => {
            eprintln!("[rhino] video: seek resync after vf failed: {e:?}; trying time-pos");
            let _ = mpv.set_property("time-pos", t);
        }
    }
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
            "[rhino] video: smooth_60 off — no 60 fps vf. Enable **Preferences** → **Smooth Video (~60 FPS at 1.0×)** for VapourSynth (bundled .vpy if path is empty)."
        );
    }
}

pub fn apply_mpv_video(mpv: &Mpv, v: &mut VideoPrefs, speed_hint: Option<f64>) -> MpvVideoApply {
    let vlog = video_log();
    log_apply(v);
    let paused = mpv.get_property::<bool>("pause").unwrap_or(true);
    let use_mvtools = v.smooth_60 && mvtools_vf_eligible(mpv, speed_hint) && !paused;
    let want_60 = v.smooth_60;
    let had_vapoursynth = vf_string_has_vapoursynth(mpv);
    if !use_mvtools {
        if had_vapoursynth {
            clear_vf(mpv, vlog);
            set_auto_decode(mpv, vlog);
            resync_av_after_vf_change(mpv);
        }
        post_smooth_60_state(mpv, v, want_60, false, vlog);
        return MpvVideoApply {
            smooth_auto_off: false,
        };
    }
    if !mpv_has_open_media(mpv) {
        let disabled_60 = add_smooth_60(mpv, v, speed_hint);
        post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
        return MpvVideoApply {
            smooth_auto_off: disabled_60,
        };
    }

    set_smooth_decode(mpv, vlog);
    clear_vf(mpv, vlog);
    let disabled_60 = add_smooth_60(mpv, v, speed_hint);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    if !disabled_60 {
        resync_av_after_vf_change(mpv);
    }
    MpvVideoApply {
        smooth_auto_off: disabled_60,
    }
}

fn mpv_escape_path(p: &str) -> String {
    if p.contains(':') || p.contains(' ') || p.contains('[') {
        format!("[{p}]")
    } else {
        p.to_string()
    }
}

#[cfg(test)]
mod model_tests {
    //! [super::mvtools_vf_eligible] is the source of truth; this module mirrors the **speed** part so
    //! tests do not need an [Mpv] handle.

    use super::normalized_env_speed;
    use super::PLAYBACK_1X_EPS;

    fn mvtools_vf_wanted_for_speed(s: f64) -> bool {
        let t = normalized_env_speed(s);
        (t - 1.0).abs() <= PLAYBACK_1X_EPS
    }

    /// When the graph **should** include `vapoursynth` (pref on + ~1.0×) but the string does not, an
    /// [apply_mpv_video] (or [super::reapply_60_if_still_missing] after load) is the way to fix it — not a timer.
    fn graph_lacks_script_while_wanted(
        smooth_pref: bool,
        playback_speed: f64,
        vf_has_vapoursynth: bool,
    ) -> bool {
        smooth_pref && mvtools_vf_wanted_for_speed(playback_speed) && !vf_has_vapoursynth
    }

    #[test]
    fn bundled_script_only_at_1x() {
        assert!(mvtools_vf_wanted_for_speed(1.0));
        assert!(!mvtools_vf_wanted_for_speed(1.5));
        assert!(!mvtools_vf_wanted_for_speed(2.0));
        assert!(!mvtools_vf_wanted_for_speed(8.0));
    }

    #[test]
    fn sped_up_does_not_require_vapoursynth_in_vf() {
        assert!(!graph_lacks_script_while_wanted(true, 1.5, false));
        assert!(!graph_lacks_script_while_wanted(true, 2.0, false));
        assert!(!graph_lacks_script_while_wanted(true, 8.0, false));
    }

    #[test]
    fn at_1x_pref_on_missing_vf_is_stale_graph() {
        assert!(graph_lacks_script_while_wanted(true, 1.0, false));
        assert!(!graph_lacks_script_while_wanted(true, 1.0, true));
        assert!(!graph_lacks_script_while_wanted(false, 1.0, false));
    }
}
