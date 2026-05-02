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
    clear_vf(mpv, vlog);
    set_auto_decode(mpv, vlog);
    true
}
pub fn apply_mpv_video_init(mpv: &Mpv, v: &mut VideoPrefs) -> MpvVideoApply {
    apply_mpv_video_impl(mpv, v, None, None)
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

pub fn apply_mpv_video(b: &MpvBundle, v: &mut VideoPrefs, speed_hint: Option<f64>) -> MpvVideoApply {
    apply_mpv_video_impl(&b.mpv, v, speed_hint, None)
}

/// Runs [apply_mpv_video_impl] with **`pause=no`** assumed for MVTools eligibility — mpv's **`pause`**
/// property can lag **`observe_property(`pause`)`** right after unpause, so the normal path would skip
/// attaching Smooth even though playback has resumed.
pub fn apply_mpv_video_after_transport_unpause(
    b: &MpvBundle,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
) -> MpvVideoApply {
    apply_mpv_video_impl(&b.mpv, v, speed_hint, Some(false))
}

fn apply_mpv_video_impl(
    mpv: &Mpv,
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
        if had_vapoursynth && !keep_vf_during_pause {
            clear_vf(mpv, vlog);
            set_auto_decode(mpv, vlog);
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

    if had_vapoursynth && smooth_vf_matches_loaded_prefs(mpv, v) {
        match speed_hint {
            Some(s) => set_playback_speed_env(s),
            None => set_playback_speed_env_from_mpv(mpv),
        }
        set_source_fps_env_from_mpv(mpv);
        post_smooth_60_state(mpv, v, want_60, false, vlog);
        return MpvVideoApply::default();
    }

    // Leave hwdec / vd-lavc-dr unchanged when attaching VS (default hwdec=auto is fine on typical stacks).
    clear_vf(mpv, vlog);
    let disabled_60 = add_smooth_60(mpv, v, speed_hint);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    MpvVideoApply {
        smooth_auto_off: disabled_60,
    }
}

/// Wrap paths for mpv `vf` / `vapoursynth:file=` when they contain characters that split sub-options
/// (`:`, `,`, `=`) or start a bracket string (`[`, `]`, space). Inside `[…]`, `\` and `]` are escaped
/// per mpv’s string rules so a trailing `]` in a path does not truncate the filter.
fn mpv_escape_path(p: &str) -> String {
    let needs_brackets = p.contains(':')
        || p.contains(' ')
        || p.contains('[')
        || p.contains(']')
        || p.contains(',')
        || p.contains('=')
        || p.contains('\\');
    if !needs_brackets {
        return p.to_string();
    }
    let mut inner = String::with_capacity(p.len() + 8);
    for ch in p.chars() {
        match ch {
            '\\' => inner.push_str(r"\\"),
            ']' => inner.push_str(r"\]"),
            _ => inner.push(ch),
        }
    }
    format!("[{inner}]")
}

#[cfg(test)]
mod mpv_escape_path_tests {
    use super::mpv_escape_path;

    #[test]
    fn unix_path_without_meta_is_unchanged() {
        assert_eq!(
            mpv_escape_path("/home/u/vs/rhino_60_mvtools.vpy"),
            "/home/u/vs/rhino_60_mvtools.vpy"
        );
    }

    #[test]
    fn space_colon_eq_comma_use_brackets() {
        assert_eq!(
            mpv_escape_path("/a b/c:d=e,f.vpy"),
            r"[/a b/c:d=e,f.vpy]"
        );
    }

    #[test]
    fn close_bracket_is_escaped_inside_brackets() {
        assert_eq!(mpv_escape_path(r"/x]y.vpy"), r"[/x\]y.vpy]");
    }

    #[test]
    fn backslash_doubled_inside_brackets() {
        assert_eq!(mpv_escape_path(r"/a\b.vpy"), r"[/a\\b.vpy]");
    }
}
