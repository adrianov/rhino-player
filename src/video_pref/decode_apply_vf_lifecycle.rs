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

/// Strip every **`vapoursynth`** node from the **`vf`** chain. Returns whether at least one remove succeeded.
fn remove_vapoursynth_vf(mpv: &Mpv, vlog: bool) -> bool {
    let mut any = false;
    for attempt in 0..8 {
        let specs = vapoursynth_vf_remove_specs(mpv);
        if specs.is_empty() {
            break;
        }
        for spec in specs {
            match mpv.command("vf", &["remove", spec.as_str()]) {
                Ok(()) => {
                    any = true;
                    if vlog {
                        eprintln!("[rhino] video: vf remove ok ({spec})");
                    }
                }
                Err(e) => {
                    eprintln!("[rhino] video: vf remove {spec} failed (attempt {}): {e:?}", attempt + 1);
                }
            }
        }
    }
    if vf_chain_has_vapoursynth(mpv) {
        eprintln!("[rhino] video: vapoursynth vf still present after vf remove");
    } else {
        forget_bundled_me_budget_vf_apply();
    }
    any
}

/// Plain playback after Smooth **off** / **`vf`** strip — prefer [remove_vapoursynth_vf] once vapoursynth is gone.
///
/// **`vo=libmpv`**: **`display-resample`** + **`report_swap`** (Linux **EGL** / **GLArea** + macOS **CVDisplayLink**).
/// Fallback **`audio`** + swap gate off if **`display-resample`** fails.
/// Vapoursynth teardown runs inside **`with_macos_vf_teardown`** when a bundle is passed (macOS).
fn restore_non_smooth_present_opts(mpv: &Mpv) {
    let _ = mpv.set_property("interpolation", "no");
    if mpv.set_property("video-sync", "display-resample").is_ok() {
        smooth_vf_swap_timing_set(true);
    } else {
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
    let had_vapoursynth = vf_chain_has_vapoursynth(mpv);
    let inner = || {
        if had_vapoursynth {
            remove_vapoursynth_vf(mpv, vlog);
        } else if mpv
            .get_property::<String>("vf")
            .is_ok_and(|s| !s.trim().is_empty())
        {
            if let Err(e) = mpv.command("vf", &["clr", ""]) {
                eprintln!("[rhino] video: vf clr failed: {e:?}");
            } else if vlog {
                eprintln!("[rhino] video: vf clr ok (non-vapoursynth chain)");
            }
        }
        restore_non_smooth_present_opts(mpv);
    };
    #[cfg(target_os = "macos")]
    {
        if let Some(b) = bundle {
            b.with_macos_vf_teardown(inner);
            sync_bluray_deinterlace_mpv(mpv, Some(b));
            b.macos_ping_render_context();
            b.macos_mark_display_pending();
        } else {
            inner();
            sync_bluray_deinterlace_mpv(mpv, None);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        inner();
        sync_bluray_deinterlace_mpv(mpv, bundle);
    }
    if had_vapoursynth {
        if let Some(b) = bundle {
            b.set_smooth_vf_stripped_this_open(true);
            b.clear_smooth_vf_reload_attempted();
        }
    }
}

/// Clear **`vf vapoursynth`** before mpv **`loadfile`** replaces media so the new file is not decoded
/// through the previous clip's warm script (avoids wrong **`RHINO_SOURCE_FPS`** / duplicate preset lines).
pub fn strip_vapoursynth_before_replace_media(b: &MpvBundle) {
    if !vf_chain_has_vapoursynth(&b.mpv) {
        return;
    }
    clear_vf(&b.mpv, Some(b), video_log());
}

fn log_vf_diagnostics(mpv: &Mpv, vlog: bool) {
    use std::sync::Mutex;
    static LAST_VF_LOG: Mutex<Option<String>> = Mutex::new(None);
    let line = match mpv.get_property::<String>("vf") {
        Ok(s) if !s.is_empty() => format!("[rhino] video: mpv property `vf` = {s:?}"),
        Ok(_) => "[rhino] video: mpv property `vf` is empty (no file, or not applied yet)".to_string(),
        Err(e) => format!("[rhino] video: could not read mpv property `vf`: {e:?}"),
    };
    let mut last = LAST_VF_LOG.lock().unwrap_or_else(|e| e.into_inner());
    if !vlog && *last == Some(line.clone()) {
        return;
    }
    *last = Some(line.clone());
    eprintln!("{line}");
    if vlog {
        if let Ok(s) = mpv.get_property::<String>("video-sync") {
            eprintln!("[rhino] video: (verbose) video-sync = {s:?}");
        }
    }
}

/// Drop the vapoursynth `vf` immediately before a **seek** (or similar position jump) when it is
/// still present so mpv can decode a real frame — especially while **paused**. Plain pause/unpause
/// does not call this.
pub fn unload_smooth_on_pause(mpv: &Mpv, bundle: Option<&MpvBundle>) -> bool {
    mark_smooth_cadence_unstable_after_seek_if_disc(mpv);
    if !vf_chain_has_vapoursynth(mpv) {
        return false;
    }
    let vlog = video_log();
    clear_vf(mpv, bundle, vlog);
    set_auto_decode(mpv, vlog);
    true
}

/// Interleaved / unstable cadence: strip VapourSynth; mpv **display-resample** only.
fn apply_interleaved_display_resample(mpv: &Mpv, bundle: Option<&MpvBundle>, vlog: bool) {
    if vf_chain_has_vapoursynth(mpv) {
        clear_vf(mpv, bundle, vlog);
    } else {
        sync_bluray_deinterlace_mpv(mpv, bundle);
    }
    apply_source_fps_env(None);
    apply_smooth_vf_present_opts(mpv);
    if bluray_playback_active(mpv, bundle) {
        eprintln!(
            "[rhino] video: Blu-ray smooth deferred — display-resample until cadence is known or stable"
        );
    } else {
        eprintln!(
            "[rhino] video: unstable frame cadence — Smooth uses mpv display-resample (no VapourSynth)"
        );
    }
}
