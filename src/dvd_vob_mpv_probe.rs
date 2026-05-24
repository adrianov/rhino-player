//! Per-`.vob` duration via a headless libmpv instance for the DVD unified timeline.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use libmpv2::events::Event;
use libmpv2::mpv_end_file_reason;
use libmpv2::Mpv;

use crate::dvd_vob_timeline::MAX_VOB_DUR_SEC;

static CACHE: Mutex<Option<HashMap<String, f64>>> = Mutex::new(None);

const PROBE_WAIT_SECS: u64 = 20;

fn cache_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn cache_get(key: &str) -> Option<Option<f64>> {
    let guard = CACHE.lock().ok()?;
    let map = guard.as_ref()?;
    map.get(key).copied().map(Some)
}

fn cache_set(key: String, dur: Option<f64>) {
    let Ok(mut guard) = CACHE.lock() else {
        return;
    };
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    if let Some(map) = guard.as_mut() {
        map.insert(key, dur.unwrap_or(f64::NAN));
    }
}

fn valid_duration(d: f64) -> bool {
    d.is_finite() && d > 0.0 && d <= MAX_VOB_DUR_SEC
}

fn read_duration(m: &Mpv) -> Option<f64> {
    m.get_property::<f64>("duration")
        .ok()
        .filter(|d| valid_duration(*d))
}

fn drain_events(m: &mut Mpv) {
    while m.wait_event(0.0).is_some() {}
}

fn new_probe_mpv() -> Option<Mpv> {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }
    Mpv::with_initializer(|i| {
        i.set_option("vo", "null")?;
        i.set_option("ao", "null")?;
        let _ = i.set_option("vid", "no");
        let _ = i.set_option("sid", "no");
        let _ = i.set_option("load-scripts", false);
        let _ = i.set_option("resume-playback", false);
        let _ = i.set_option("length", 0.0f64); // stop after open; duration already known from demuxer header
        let _ = i.set_option("demuxer-readahead-secs", 0.0f64);
        let _ = i.set_option("demuxer-max-bytes", "128KiB");
        let _ = i.set_option("autoload-files", "no");
        let _ = i.set_option("audio-file-auto", "no");
        let _ = i.set_option("sub-auto", "no");
        let _ = i.set_option("hr-seek", "yes");
        Ok(())
    })
    .map_err(|e| {
        crate::dvd_vob_log::dvd_seek_log(format!("mpv probe init failed: {e}"));
    })
    .ok()
}

fn wait_vob_duration(m: &mut Mpv, deadline: Instant) -> Option<f64> {
    loop {
        if let Some(d) = read_duration(m) {
            return Some(d);
        }
        if Instant::now() >= deadline {
            return None;
        }
        match m.wait_event(0.05) {
            Some(Ok(Event::FileLoaded)) => {
                if let Some(d) = read_duration(m) {
                    return Some(d);
                }
            }
            Some(Ok(Event::EndFile(r))) => {
                if r == mpv_end_file_reason::Error {
                    return None;
                }
                return read_duration(m);
            }
            Some(Err(_)) => drain_events(m),
            Some(Ok(_)) | None => {}
        }
    }
}

fn probe_with_mpv(path: &Path) -> Option<f64> {
    let mut m = new_probe_mpv()?;
    let src = path.to_str()?;
    drain_events(&mut m);
    if m.command("loadfile", &[src, "replace"]).is_err() {
        crate::dvd_vob_log::dvd_seek_log(format!("mpv probe loadfile failed {}", path.display()));
        return None;
    }
    let started = Instant::now();
    let deadline = started + Duration::from_secs(PROBE_WAIT_SECS);
    let dur = wait_vob_duration(&mut m, deadline);
    if dur.is_none() {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "mpv probe no duration {} (after {:.1}s)",
            path.display(),
            started.elapsed().as_secs_f64()
        ));
    }
    dur
}

/// Whole-file duration in seconds from libmpv, with an in-process cache per canonical path.
pub fn probe_vob_duration(path: &Path) -> Option<f64> {
    if !path.is_file() {
        return None;
    }
    let key = cache_key(path);
    if let Some(hit) = cache_get(&key) {
        return hit.filter(|d| valid_duration(*d));
    }
    let dur = probe_with_mpv(path);
    cache_set(key, dur);
    dur
}

#[cfg(test)]
pub(crate) fn clear_probe_cache() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(HashMap::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpv_probe_real_dvd5_vob() {
        let vob = Path::new(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD5/VIDEO_TS/VTS_02_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        clear_probe_cache();
        let started = Instant::now();
        let d = probe_vob_duration(vob).expect("duration");
        assert!(d > 1000.0, "expected ~1130s part, got {d}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "probe took {:.1}s",
            started.elapsed().as_secs_f64()
        );
    }
}
