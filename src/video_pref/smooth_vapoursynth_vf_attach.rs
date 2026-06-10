/// Attaches **`vf=vapoursynth`** with fixed **`buffered-frames`** and **`concurrent-frames=auto`**.
///
/// Bundled ME px² is passed via **`RHINO_SMOOTH_MAX_AREA`** (**[crate::paths::publish_smooth_me_budget_env]**),
/// which the `.vpy` reads with libc **`getenv`** (same as other **`RHINO_*`** vars).
///
/// One `vf add` attempt only — a rejected add is never transient here (missing install, or an
/// unpinned VSScript runtime after a filter destroy; see [pin_vsscript_python]), so retries
/// just spam identical errors and delay the failure handling.
fn log_vf_add_fail_state(
    mpv: &libmpv2::Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    err: &libmpv2::Error,
) {
    let pause = mpv.get_property::<bool>("pause").ok();
    let core_idle = mpv.get_property::<bool>("core-idle").ok();
    let pos = mpv.get_property::<f64>("time-pos").ok();
    let dur = mpv.get_property::<f64>("duration").ok();
    let vf = mpv
        .get_property::<String>("vf")
        .ok()
        .map(|s| if s.trim().is_empty() { "<empty>".into() } else { s });
    let path = crate::media_probe::local_file_from_mpv(mpv)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<none>".into());
    let resume_pending = bundle
        .map(|b| b.resume_seek_pending())
        .unwrap_or(false);
    eprintln!(
        "[rhino] video: vf add fail state err={err:?} pause={} core-idle={} resume-pending={} pos={} dur={} path={} vf={}",
        pause.map(|x| x.to_string()).unwrap_or_else(|| "?".into()),
        core_idle.map(|x| x.to_string()).unwrap_or_else(|| "?".into()),
        resume_pending,
        pos.map(|x| format!("{x:.3}")).unwrap_or_else(|| "?".into()),
        dur.map(|x| format!("{x:.3}")).unwrap_or_else(|| "?".into()),
        path,
        vf.unwrap_or_else(|| "?".into()),
    );
}

pub(crate) fn smooth_vapoursynth_vf_try_attach(
    mpv: &libmpv2::Mpv,
    script_path_escaped: &str,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> bool {
    if vf_chain_has_vapoursynth(mpv) {
        return true;
    }
    if bundle.is_some_and(|b| b.smooth_vf_attach_pending()) {
        eprintln!("[rhino] video: vf add skipped (vapoursynth attach already in flight)");
        return true;
    }
    pin_vsscript_python();
    if let Some(b) = bundle {
        b.set_smooth_vf_attach_pending(true);
        let ok = {
            #[cfg(target_os = "macos")]
            {
                b.with_macos_vf_teardown(|| vf_add_once(mpv, script_path_escaped, Some(b)))
            }
            #[cfg(not(target_os = "macos"))]
            {
                vf_add_once(mpv, script_path_escaped, Some(b))
            }
        };
        b.set_smooth_vf_attach_pending(false);
        return ok;
    }
    vf_add_once(mpv, script_path_escaped, None)
}

fn vf_add_once(
    mpv: &libmpv2::Mpv,
    script_path_escaped: &str,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> bool {
    let bf = SMOOTH_VF_BUFFERED_FRAMES;
    let spec = format!(
        "vapoursynth:file={script_path_escaped}:buffered-frames={bf}:concurrent-frames=auto",
    );
    match mpv.command("vf", &["add", spec.as_str()]) {
        Ok(()) => {
            eprintln!("[rhino] video: vf add vapoursynth command accepted");
            if let Some(b) = bundle {
                b.clear_smooth_vf_stripped_this_open();
                b.clear_smooth_vf_reload_attempted();
            }
            #[cfg(target_os = "macos")]
            crate::app::schedule_macos_shell_refresh_after_vf();
            true
        }
        Err(e) => {
            if vf_chain_has_vapoursynth(mpv) {
                eprintln!(
                    "[rhino] video: vf add vapoursynth returned {e:?} but filter is present — keeping Smooth on"
                );
                return true;
            }
            log_vf_add_fail_state(mpv, bundle, &e);
            eprintln!(
                "[rhino] video: vf add vapoursynth failed: {e:?} \
                 (install VapourSynth + vapoursynth-mvtools; on macOS run `brew install vapoursynth vapoursynth-mvtools`)."
            );
            false
        }
    }
}
