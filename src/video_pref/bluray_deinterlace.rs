// Blu-ray interlaced content: mpv **bwdif** Bob deinterlace (mode=1 → ~60 fps fields).

/// mpv `vf` label for the conditional Bob deinterlace filter.
pub(crate) const DEINT_VF_LABEL: &str = "rhino-deint";

/// `vf` subchain: **mode=1** Bob; **deint=interlaced** skips progressive frames (libavfilter).
/// mpv 0.41 does not accept `cond=` in `--vf` / `vf add` (unlike some `mpv.conf` examples).
const DEINT_VF_SPEC: &str = "@rhino-deint:bwdif=mode=1:deint=interlaced";

#[must_use]
pub(crate) fn bluray_deinterlace_in_vf(vf: &str) -> bool {
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

/// True when Rhino should keep the conditional Bob **vf** on the current item.
///
/// **deint=interlaced** applies **bwdif** only on interlaced frames; we attach whenever Blu-ray
/// is open so folder/`loadfile` titles work before stream metadata is fully known.
#[must_use]
pub(crate) fn wants_bluray_bob_deinterlace(mpv: &Mpv, bundle: Option<&MpvBundle>) -> bool {
    bluray_playback_active(mpv, bundle)
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

/// Attach conditional **bwdif** Bob deinterlace; returns `false` on mpv error.
pub(crate) fn attach_bluray_deinterlace(mpv: &Mpv) -> bool {
    if bluray_deinterlace_in_vf(
        &mpv.get_property::<String>("vf").unwrap_or_default(),
    ) {
        return true;
    }
    ensure_hwdec_vf_copy(mpv);
    match mpv.command("vf", &["add", DEINT_VF_SPEC]) {
        Ok(()) => {
            eprintln!("[rhino] video: Blu-ray Bob deinterlace attached (bwdif mode=1 when interlaced)");
            true
        }
        Err(e) => {
            eprintln!(
                "[rhino] video: Blu-ray deinterlace vf add failed: {e:?} (mpv COMMAND — bad filter string or no video yet)"
            );
            false
        }
    }
}

fn detach_bluray_deinterlace(mpv: &Mpv) {
    if !bluray_deinterlace_in_vf(&mpv.get_property::<String>("vf").unwrap_or_default()) {
        return;
    }
    let label = format!("@{DEINT_VF_LABEL}");
    if let Err(e) = mpv.command("vf", &["remove", &label]) {
        eprintln!("[rhino] video: Blu-ray deinterlace vf remove failed: {e:?}");
    } else if video_log() {
        eprintln!("[rhino] video: Blu-ray Bob deinterlace removed");
    }
}

/// Ensure conditional Bob deinterlace is present on Blu-ray, absent elsewhere.
pub(crate) fn sync_bluray_deinterlace_mpv(mpv: &Mpv, bundle: Option<&MpvBundle>) {
    let want = wants_bluray_bob_deinterlace(mpv, bundle);
    let vf = mpv.get_property::<String>("vf").unwrap_or_default();
    let has = bluray_deinterlace_in_vf(&vf);
    if want && !has {
        let _ = attach_bluray_deinterlace(mpv);
    } else if !want && has {
        detach_bluray_deinterlace(mpv);
    }
}
