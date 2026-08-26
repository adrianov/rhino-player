// vf diagnostics logging (dedup-gated) shared by the Smooth apply/teardown paths.

fn log_vf_diagnostics(mpv: &Mpv, vlog: bool) {
    use std::sync::Mutex;
    static LAST_VF_LOG: Mutex<Option<String>> = Mutex::new(None);
    let line = vf_property_line(mpv);
    let mut last = LAST_VF_LOG.lock().unwrap_or_else(|e| e.into_inner());
    if !vlog && *last == Some(line.clone()) {
        return;
    }
    *last = Some(line.clone());
    eprintln!("{line}");
    if vlog {
        log_verbose_video_sync(mpv);
    }
}

fn vf_property_line(mpv: &Mpv) -> String {
    match mpv.get_property::<String>("vf") {
        Ok(s) if !s.is_empty() => format!("[rhino] video: mpv property `vf` = {s:?}"),
        Ok(_) => {
            "[rhino] video: mpv property `vf` is empty (no file, or not applied yet)".to_string()
        }
        Err(e) => format!("[rhino] video: could not read mpv property `vf`: {e:?}"),
    }
}

fn log_verbose_video_sync(mpv: &Mpv) {
    if let Ok(s) = mpv.get_property::<String>("video-sync") {
        eprintln!("[rhino] video: (verbose) video-sync = {s:?}");
    }
}
