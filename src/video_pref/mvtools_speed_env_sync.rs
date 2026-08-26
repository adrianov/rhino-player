// Playback-speed env sync: `RHINO_PLAYBACK_SPEED` publishing, ~1.0× eligibility, and the
// decision whether the Smooth `vf` needs a rebuild (included by `mvtools_speed_vf_setup.rs`).

/// “≈1.0×” band: bundled mvtools [vf] eligibility and env comparison use this tolerance.
const PLAYBACK_1X_EPS: f64 = 0.001;

/// Same string [mpv] and the VapourSynth script use for `RHINO_PLAYBACK_SPEED`.
fn normalized_env_speed(s: f64) -> f64 {
    if !s.is_finite() {
        return 1.0;
    }
    // Cap at the fastest fixed UI step so env matches mpv (see playback_speed::MAX_FIXED_SPEED).
    let s = if s > 0.01 && s <= MAX_FIXED_SPEED {
        s
    } else {
        1.0
    };
    (s * 10.0).round() / 10.0
}

/// Set [paths::RHINO_PLAYBACK_SPEED_VAR] to `speed` (e.g. value just sent with [Mpv] `set_property`,
/// before [get_property] reflects it — avoids a stale env when rebuilding the [vf]).
pub fn set_playback_speed_env(speed: f64) {
    let t = normalized_env_speed(speed);
    std::env::set_var(RHINO_PLAYBACK_SPEED_VAR, format!("{t}"));
}

/// Set [paths::RHINO_PLAYBACK_SPEED_VAR] from [libmpv2::Mpv] `speed` (defaults to `1.0`). Used before
/// loading the VapourSynth filter so the bundled script matches interpolation to (source fps × speed).
pub fn set_playback_speed_env_from_mpv(mpv: &Mpv) {
    let s = match mpv.get_property::<f64>("speed") {
        Ok(v) if v.is_finite() => v,
        _ => 1.0,
    };
    set_playback_speed_env(s);
}

/// Bundled mvtools / FlowFPS is only used at **1.0×** (no speed-up). If [mpv] `speed` is not ~1, the
/// [vf] is omitted; **Smooth 60** pref may stay on for when the user returns to 1.0×.
/// [speed_hint] is used when [Some] (e.g. header row) so we do not read [get_property] before it matches
/// the value just sent with [set_property] — that race skipped re-adding the [vf] when going 1.5/2.0 → 1.0.
/// Bundled/custom VapourSynth graph — skipped when interleaved cadence needs display-resample only.
pub(crate) fn smooth_wants_vapoursynth_vf(
    mpv: &Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    speed_hint: Option<f64>,
) -> bool {
    !smooth_load_hold_active()
        && mvtools_vf_eligible(mpv, speed_hint)
        && !smooth_prefers_display_resample_bundle(mpv, bundle)
}

pub(crate) fn mvtools_vf_eligible(mpv: &Mpv, speed_hint: Option<f64>) -> bool {
    let s = match speed_hint {
        Some(x) if x.is_finite() => normalized_env_speed(x),
        _ => match mpv.get_property::<f64>("speed") {
            Ok(v) if v.is_finite() => normalized_env_speed(v),
            _ => 1.0,
        },
    };
    (s - 1.0).abs() <= PLAYBACK_1X_EPS
}

/// `true` when the process env disagrees with current [mpv] `speed` (e.g. [vf] added before resume
/// applied playback speed, or UI set `speed` before the resync read ran).
pub fn needs_playback_speed_env_resync(mpv: &Mpv) -> bool {
    let want = {
        let s = match mpv.get_property::<f64>("speed") {
            Ok(v) if v.is_finite() => v,
            _ => 1.0,
        };
        normalized_env_speed(s)
    };
    let have = std::env::var(RHINO_PLAYBACK_SPEED_VAR)
        .ok()
        .and_then(|t| t.parse::<f64>().ok())
        .map(normalized_env_speed)
        .unwrap_or(1.0);
    (have - want).abs() > PLAYBACK_1X_EPS
}

/// True when the loaded `vf`/decode state no longer matches what [apply_mpv_video] would install:
/// presence vs [smooth_wants_vapoursynth_vf] (want vapoursynth only at ~1.0×; strip when sped up),
/// or prefs/script/buffer mismatch ([vf_smooth_matches_prefs]).
fn smooth_vf_state_differs(mpv: &Mpv, b: &crate::mpv_embed::MpvBundle, v: &VideoPrefs) -> bool {
    let want_mvtools = smooth_wants_vapoursynth_vf(mpv, Some(b), None);
    let has = vf_chain_has_vapoursynth(mpv);
    let graph_ok = !has || vf_smooth_matches_prefs(mpv, v, Some(b));
    !(want_mvtools == has && graph_ok)
}

/// If **Smooth 60** is on and media is open, runs [apply_mpv_video] when the decode/`vf` state should
/// change: env/`speed` mismatch, or the graph does not match [mvtools_vf_eligible] (want **vapoursynth**
/// only at ~1.0×; strip when sped up), or the loaded `vf` does not match prefs/script/buffer options.
/// Returns the same shape as [apply_mpv_video].
pub fn resync_smooth_if_speed_mismatch(
    player: &std::rc::Rc<std::cell::RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    v: &mut VideoPrefs,
) -> MpvVideoApply {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return MpvVideoApply::default();
    };
    let mpv = &b.mpv;
    if !v.smooth_60 || !mpv_has_open_media(mpv) || b.smooth_vf_attach_pending() {
        return MpvVideoApply::default();
    }
    if smooth_vf_state_differs(mpv, b, v) {
        drop(g);
        return apply_mpv_video(player, v, None);
    }
    if needs_playback_speed_env_resync(mpv) {
        set_playback_speed_env_from_mpv(mpv);
    }
    MpvVideoApply::default()
}

/// After [libmpv2::Mpv] `speed` changes: re-run [apply_mpv_video] so `vf` / decode match
/// (mvtools only at ~1.0×; see [mvtools_vf_eligible]).
/// Pass [speed_hint] with the `speed` you just set in mpv to avoid a **get_property** race; use `None` to
/// read the current [mpv] value.
pub fn refresh_smooth_for_playback_speed(
    player: &std::rc::Rc<std::cell::RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
) -> MpvVideoApply {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return MpvVideoApply::default();
    };
    let mpv = &b.mpv;
    if !v.smooth_60 || !mpv_has_open_media(mpv) {
        return MpvVideoApply::default();
    }
    eprintln!("[rhino] video: video pipeline resync for playback speed");
    match speed_hint {
        Some(s) => set_playback_speed_env(s),
        None => set_playback_speed_env_from_mpv(mpv),
    }
    drop(g);
    apply_mpv_video(player, v, speed_hint)
}
