// Interlaced HD / Blu-ray: mpv **bwdif** Bob deinterlace (mode=1 → ~60 fps fields).
//
// Narrow orchestration API for callers:
// - [bob_prepare_apply] / [bob_finish_apply] — `apply_mpv_video` begin/end
// - [sync_bob_deinterlace_mpv] / [sync_bluray_deinterlace_mpv] — after `vf` clear / interleaved
// - [bob_blocks_smooth_vf] — Smooth VapourSynth eligibility
// - [bob_vf_matches_want] — Smooth `vf` already-matches check
// - [bob_needs_apply_when_smooth_off] — transport resync when Smooth is off

/// mpv `vf` label for the conditional Bob deinterlace filter.
pub(crate) const DEINT_VF_LABEL: &str = "rhino-deint";

/// `vf` subchain: **mode=1** Bob; **deint=interlaced** skips progressive frames (libavfilter).
/// mpv 0.41 does not accept `cond=` in `--vf` / `vf add` (unlike some `mpv.conf` examples).
const DEINT_VF_SPEC: &str = "@rhino-deint:bwdif=mode=1:deint=interlaced";

/// Decode height band treated as 1080i-class (allows slight crop / encode padding).
const HD_INTERLACE_H_MIN: i64 = 1000;
const HD_INTERLACE_H_MAX: i64 = 1200;

// After Bob runs, mpv reports progressive `video-frame-info` — sticky path keeps Bob for this open.
thread_local! {
    static LOCAL_1080I_PATH: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

#[must_use]
pub(crate) fn bob_deinterlace_in_vf(vf: &str) -> bool {
    let v = vf.to_ascii_lowercase();
    v.contains(DEINT_VF_LABEL) && v.contains("bwdif")
}

#[must_use]
pub(crate) fn bluray_playback_active(mpv: &Mpv, bundle: Option<&MpvBundle>) -> bool {
    if mpv
        .get_property::<String>("path")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some_and(|s| {
            let l = s.trim().to_ascii_lowercase();
            l.starts_with("bd://") || l.starts_with("bluray://")
        })
    {
        return true;
    }
    me_budget_local_path(mpv, bundle)
        .is_some_and(|p| crate::video_ext::is_bluray_disc_path(&p))
}

fn decode_height_px(mpv: &Mpv) -> Option<i64> {
    mpv.get_property::<i64>("video-params/h")
        .or_else(|_| mpv.get_property::<i64>("height"))
        .ok()
        .filter(|&h| h > 0)
}

fn hd_interlace_height(h: i64) -> bool {
    (HD_INTERLACE_H_MIN..=HD_INTERLACE_H_MAX).contains(&h)
}

fn decode_height_hd_interlace_band(mpv: &Mpv) -> bool {
    decode_height_px(mpv).is_some_and(hd_interlace_height)
}

fn open_media_key(mpv: &Mpv, bundle: Option<&MpvBundle>) -> Option<String> {
    me_budget_local_path(mpv, bundle)
        .and_then(|p| p.to_str().map(str::to_owned))
        .or_else(|| {
            mpv.get_property::<String>("path")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
}

fn sticky_local_1080i(path: &str) -> bool {
    LOCAL_1080I_PATH.with(|slot| {
        let mut g = slot.borrow_mut();
        match g.as_deref() {
            Some(prev) if prev == path => true,
            Some(_) => {
                *g = None;
                false
            }
            None => false,
        }
    })
}

fn note_local_1080i(path: &str) {
    LOCAL_1080I_PATH.with(|slot| {
        *slot.borrow_mut() = Some(path.to_owned());
    });
}

/// True when the open item is local ~1080 interlaced (sticky after first detect).
#[must_use]
fn local_hd_interlaced(mpv: &Mpv, bundle: Option<&MpvBundle>) -> bool {
    if bluray_playback_active(mpv, bundle) {
        return false;
    }
    let Some(key) = open_media_key(mpv, bundle) else {
        return false;
    };
    if sticky_local_1080i(&key) {
        return true;
    }
    // Unavailable until the first frame decodes — not the same as progressive `false`.
    // After Bob, the flag flips to progressive; sticky path above keeps this open armed.
    if mpv.get_property::<bool>("video-frame-info/interlaced") != Ok(true) {
        return false;
    }
    if !decode_height_hd_interlace_band(mpv) {
        return false;
    }
    note_local_1080i(&key);
    true
}

#[must_use]
fn wants_bob_deinterlace(mpv: &Mpv, bundle: Option<&MpvBundle>) -> bool {
    bluray_playback_active(mpv, bundle) || local_hd_interlaced(mpv, bundle)
}

/// Local 1080i: Bob alone supplies ~60 field frames — Smooth VapourSynth must not attach.
#[must_use]
pub(crate) fn bob_blocks_smooth_vf(mpv: &Mpv, bundle: Option<&MpvBundle>) -> bool {
    local_hd_interlaced(mpv, bundle)
}

/// Smooth `vf` match: Bob label present whenever Bob is wanted for this open.
#[must_use]
pub(crate) fn bob_vf_matches_want(mpv: &Mpv, bundle: Option<&MpvBundle>, vf: &str) -> bool {
    !wants_bob_deinterlace(mpv, bundle) || bob_deinterlace_in_vf(vf)
}

/// When Smooth is off, still run [apply_mpv_video] for Bob attach/detach (and HD probe).
#[must_use]
pub(crate) fn bob_needs_apply_when_smooth_off(mpv: &Mpv, bundle: Option<&MpvBundle>) -> bool {
    if wants_bob_deinterlace(mpv, bundle) {
        return true;
    }
    if bob_deinterlace_in_vf(&mpv.get_property::<String>("vf").unwrap_or_default()) {
        return true;
    }
    // Probe only while HD height is known but interlaced is still unavailable (not progressive).
    decode_height_hd_interlace_band(mpv)
        && mpv
            .get_property::<bool>("video-frame-info/interlaced")
            .is_err()
}

/// Start of [apply_mpv_video] with open media: attach/detach Bob before Smooth `vf add`.
pub(crate) fn bob_prepare_apply(mpv: &Mpv, bundle: Option<&MpvBundle>) {
    sync_bob_deinterlace_mpv(mpv, bundle);
}

/// End of [apply_mpv_video] with open media: local 1080i keeps Bob-only present opts (no Smooth script).
pub(crate) fn bob_finish_apply(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    want_60: bool,
    speed_hint: Option<f64>,
    vlog: bool,
) {
    if want_60 && mvtools_vf_eligible(mpv, speed_hint) && bob_blocks_smooth_vf(mpv, bundle) {
        present_local_1080i_bob(mpv, bundle, vlog);
    }
}

fn present_local_1080i_bob(mpv: &Mpv, bundle: Option<&MpvBundle>, vlog: bool) {
    cancel_deferred_vf_swap();
    if vf_chain_has_vapoursynth(mpv) {
        // clear_vf re-syncs Bob after stripping vapoursynth.
        clear_vf(mpv, bundle, vlog);
    }
    apply_smooth_vf_present_opts(mpv);
    if video_log() {
        eprintln!(
            "[rhino] video: 1080i Bob deinterlace (~60 fields) — Smooth script skipped for this open"
        );
    }
}

/// Hardware decode must use a **-copy** path so CPU `vf` filters can read frames.
pub(crate) fn ensure_hwdec_vf_copy(mpv: &Mpv) {
    #[cfg(target_os = "macos")]
    const CANDIDATES: &[&str] = &["videotoolbox-copy", "auto-copy", "no"];
    #[cfg(target_os = "linux")]
    const CANDIDATES: &[&str] = &["auto-copy", "vaapi-copy", "nvdec-copy", "no"];
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    const CANDIDATES: &[&str] = &["auto-copy", "no"];

    for mode in CANDIDATES {
        if mpv.set_property("hwdec", *mode).is_ok() {
            if video_log() {
                eprintln!("[rhino] video: (verbose) hwdec={mode} for vf (deinterlace / VapourSynth)");
            }
            return;
        }
    }
}

fn attach_bob_deinterlace(mpv: &Mpv, bundle: Option<&MpvBundle>) -> bool {
    if bob_deinterlace_in_vf(&mpv.get_property::<String>("vf").unwrap_or_default()) {
        return true;
    }
    ensure_hwdec_vf_copy(mpv);
    match mpv.command("vf", &["add", DEINT_VF_SPEC]) {
        Ok(()) => {
            let kind = if bluray_playback_active(mpv, bundle) {
                "Blu-ray"
            } else {
                "1080i"
            };
            eprintln!(
                "[rhino] video: {kind} Bob deinterlace attached (bwdif mode=1 when interlaced)"
            );
            true
        }
        Err(e) => {
            eprintln!(
                "[rhino] video: Bob deinterlace vf add failed: {e:?} (mpv COMMAND — bad filter string or no video yet)"
            );
            false
        }
    }
}

fn detach_bob_deinterlace(mpv: &Mpv) {
    if !bob_deinterlace_in_vf(&mpv.get_property::<String>("vf").unwrap_or_default()) {
        return;
    }
    let label = format!("@{DEINT_VF_LABEL}");
    if let Err(e) = mpv.command("vf", &["remove", &label]) {
        eprintln!("[rhino] video: Bob deinterlace vf remove failed: {e:?}");
    } else if video_log() {
        eprintln!("[rhino] video: Bob deinterlace removed");
    }
}

/// Ensure conditional Bob deinterlace is present when wanted, absent otherwise.
pub(crate) fn sync_bob_deinterlace_mpv(mpv: &Mpv, bundle: Option<&MpvBundle>) {
    let want = wants_bob_deinterlace(mpv, bundle);
    let has = bob_deinterlace_in_vf(&mpv.get_property::<String>("vf").unwrap_or_default());
    if want && !has {
        let _ = attach_bob_deinterlace(mpv, bundle);
    } else if !want && has {
        detach_bob_deinterlace(mpv);
    }
}

/// After `vf` clear / interleaved display-resample — same as [sync_bob_deinterlace_mpv].
/// Kept so lifecycle call sites stay free of Bob-specific naming.
pub(crate) fn sync_bluray_deinterlace_mpv(mpv: &Mpv, bundle: Option<&MpvBundle>) {
    sync_bob_deinterlace_mpv(mpv, bundle);
}
