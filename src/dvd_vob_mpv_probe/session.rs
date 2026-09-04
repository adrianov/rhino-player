// Headless mpv probe sessions for `.vob` durations (included from `dvd_vob_mpv_probe.rs`).

use std::time::{Duration, Instant};

/// Probes per background idle tick while the UI stays responsive.
pub(crate) const BG_PROBE_BATCH: usize = 8;

pub(crate) fn is_title_chain_head(path: &Path) -> bool {
    crate::dvd_entity::vob_part_id(path) == Some(1)
        && crate::dvd_entity::title_chapter_paths(path).is_some_and(|p| p.len() > 1)
}

include!("session_chain.rs");
include!("session_wait.rs");

fn drain_events(m: &mut Mpv) {
    while m.wait_event(0.0).is_some() {}
}

/// Probe-only defaults: null outputs, no scripts/subs/audio, minimal demuxer buffers.
fn apply_probe_defaults(i: libmpv2::MpvInitializer) -> Result<(), libmpv2::Error> {
    i.set_option("vo", "null")?;
    i.set_option("ao", "null")?;
    let _ = i.set_option("vid", "no");
    let _ = i.set_option("sid", "no");
    let _ = i.set_option("load-scripts", false);
    let _ = i.set_option("resume-playback", false);
    let _ = i.set_option("length", 0.0f64);
    let _ = i.set_option("demuxer-readahead-secs", 0.0f64);
    let _ = i.set_option("demuxer-max-bytes", "128KiB");
    let _ = i.set_option("autoload-files", "no");
    let _ = i.set_option("audio-file-auto", "no");
    let _ = i.set_option("sub-auto", "no");
    let _ = i.set_option("hr-seek", "yes");
    Ok(())
}

fn new_probe_mpv() -> Option<Mpv> {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }
    Mpv::with_initializer(apply_probe_defaults)
        .map_err(|e| {
            crate::dvd_vob_log::dvd_seek_log(format!("mpv probe init failed: {e}"));
        })
        .ok()
}

fn resolve_probe_duration(m: &mut Mpv, path: &Path) -> Option<f64> {
    if let Some(d) = probe_with_session(m, path).filter(|d| valid_duration(*d)) {
        return Some(d);
    }
    chain_head_duration(m, path)
}

fn log_probe_miss(path: &Path, elapsed_secs: f64) {
    crate::dvd_vob_log::dvd_seek_log(format!(
        "mpv probe no duration {} (after {elapsed_secs:.1}s)",
        path.display()
    ));
}

fn probe_with_session(m: &mut Mpv, path: &Path) -> Option<f64> {
    let src = path.to_str()?;
    if !restart_session_on(m, src, path) {
        return None;
    }
    let started = Instant::now();
    let dur = wait_vob_duration(m, started + Duration::from_secs(PROBE_WAIT_SECS));
    settle_session(m);
    if dur.is_none() {
        log_probe_miss(path, started.elapsed().as_secs_f64());
    }
    dur
}

/// Restart the session on the target file; `false` when the `loadfile` command fails.
fn restart_session_on(m: &mut Mpv, src: &str, path: &Path) -> bool {
    drain_events(m);
    let _ = m.command("stop", &[]);
    drain_events(m);
    if m.command("loadfile", &[src, "replace"]).is_err() {
        crate::dvd_vob_log::dvd_seek_log(format!("mpv probe loadfile failed {}", path.display()));
        return false;
    }
    true
}

/// Stop playback and drain pending events after a probe attempt.
fn settle_session(m: &mut Mpv) {
    let _ = m.command("stop", &[]);
    drain_events(m);
}

const PROBE_WAIT_SECS: u64 = 12;
