use libmpv2::events::Event;
use libmpv2::Mpv;

fn headless_mpv() -> Result<Mpv, String> {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }
    Mpv::with_initializer(|i| {
        i.set_option("vo", "null")?;
        i.set_option("ao", "null")?;
        i.set_option("load-scripts", false)?;
        i.set_option("idle", "yes")?;
        Ok(())
    })
    .map_err(|e| format!("{e:?}"))
}

fn probe_libmpv() -> Result<String, String> {
    let _mpv = headless_mpv().map_err(|e| format!("init failed: {e}"))?;
    Ok("initialized (vo=null)".into())
}

fn probe_vapoursynth_filter() -> Result<String, String> {
    let mut mpv = headless_mpv().map_err(|e| format!("init failed: {e}"))?;
    request_mpv_warn_logs(&mpv);
    classify_vs_filter_probe(
        mpv
            .command("vf", &["add", "vapoursynth=file=/nonexistent-rhino-diag.vpy"])
            .is_ok(),
        &drain_mpv_log_lines(&mut mpv),
    )
}

fn request_mpv_warn_logs(mpv: &Mpv) {
    let Ok(level) = std::ffi::CString::new("warn") else {
        return;
    };
    let r = unsafe { libmpv2_sys::mpv_request_log_messages(mpv.ctx.as_ptr(), level.as_ptr()) };
    if r < 0 {
        eprintln!("[rhino] diagnostics: mpv_request_log_messages failed err={r}");
    }
}

fn drain_mpv_log_lines(mpv: &mut Mpv) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(ev) = mpv.wait_event(0.0) {
        if let Ok(Event::LogMessage { prefix, text, .. }) = ev {
            lines.push(format!("{prefix}: {}", text.trim_end()));
        }
    }
    lines
}

/// libmpv command errors are often bare `Raw(MPV_ERROR_COMMAND)` — only trust log text.
fn classify_vs_filter_probe(cmd_ok: bool, logs: &[String]) -> Result<String, String> {
    let joined = logs.join("\n").to_ascii_lowercase();
    if filter_missing_in_logs(&joined) {
        return Err(format!(
            "not in this libmpv — need vapoursynth-enabled mpv/libmpv ({})",
            summarize_logs(logs)
        ));
    }
    if cmd_ok {
        return Ok("vf add accepted (unexpected without media)".into());
    }
    if filter_present_in_logs(&joined) {
        return Ok(format!(
            "present (add rejected as expected without media: {})",
            summarize_logs(logs)
        ));
    }
    Err(format!(
        "inconclusive command failure without filter log evidence ({})",
        summarize_logs(logs)
    ))
}

fn summarize_logs(logs: &[String]) -> String {
    if logs.is_empty() {
        return "no mpv log lines".into();
    }
    logs.join(" | ")
}

fn filter_missing_in_logs(lower: &str) -> bool {
    lower.contains("isn't supported")
        || lower.contains("is not supported")
        || lower.contains("option vapoursynth not found")
        || lower.contains("filter not found")
        || lower.contains("no such filter")
        || lower.contains("didn't find filter")
        || (lower.contains("vapoursynth") && lower.contains("not found"))
}

fn filter_present_in_logs(lower: &str) -> bool {
    !filter_missing_in_logs(lower)
        && (lower.contains("vapoursynth")
            || lower.contains("nonexistent-rhino-diag")
            || lower.contains("error opening")
            || lower.contains("could not open")
            || lower.contains("failed to open"))
}

#[cfg(test)]
mod vs_filter_probe_tests {
    use super::classify_vs_filter_probe;

    #[test]
    fn missing_filter_log_fails() {
        let logs = ["cplayer: Option vf: 'vapoursynth' isn't supported.".into()];
        assert!(classify_vs_filter_probe(false, &logs).is_err());
    }

    #[test]
    fn bare_command_failure_without_logs_fails() {
        let err = classify_vs_filter_probe(false, &[]).unwrap_err();
        assert!(err.contains("inconclusive"));
    }

    #[test]
    fn vapoursynth_log_without_missing_counts_as_present() {
        let logs = ["vapoursynth: could not open script file".into()];
        assert!(classify_vs_filter_probe(false, &logs).is_ok());
    }

    #[test]
    fn command_ok_passes() {
        assert!(classify_vs_filter_probe(true, &[]).is_ok());
    }
}
