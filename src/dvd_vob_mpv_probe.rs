//! Per-`.vob` duration via a reused headless libmpv instance for the DVD unified timeline.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use libmpv2::events::Event;
use libmpv2::mpv_end_file_reason;
use libmpv2::Mpv;

use crate::dvd_vob_timeline::MAX_VOB_DUR_SEC;

static CACHE: Mutex<Option<HashMap<String, f64>>> = Mutex::new(None);

thread_local! {
    static PROBE_MPV: RefCell<Option<Mpv>> = const { RefCell::new(None) };
}

const PROBE_WAIT_SECS: u64 = 12;
/// Probes per background idle tick while the UI stays responsive.
pub(crate) const BG_PROBE_BATCH: usize = 8;

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
    read_raw_duration(m).filter(|d| valid_duration(*d))
}

fn read_raw_duration(m: &Mpv) -> Option<f64> {
    m.get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
}

pub(crate) fn is_title_chain_head(path: &Path) -> bool {
    crate::dvd_entity::vob_part_id(path) == Some(1)
        && crate::dvd_entity::title_chapter_paths(path).is_some_and(|p| p.len() > 1)
}

/// First `.vob` in a chained title reports the whole program; derive length from siblings.
fn chain_head_duration(m: &mut Mpv, path: &Path) -> Option<f64> {
    if !is_title_chain_head(path) {
        return None;
    }
    let chapters = crate::dvd_entity::title_chapter_paths(path)?;
    let head_bytes = path.metadata().ok()?.len();
    if head_bytes == 0 {
        return None;
    }
    let mut bps: Vec<f64> = chapters
        .iter()
        .skip(1)
        .filter_map(|sib| {
            let dur = probe_with_session(m, sib)?;
            if !valid_duration(dur) {
                return None;
            }
            let bytes = sib.metadata().ok()?.len();
            (bytes > 0).then_some(bytes as f64 / dur)
        })
        .collect();
    if bps.is_empty() {
        return None;
    }
    bps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let est = head_bytes as f64 / bps[bps.len() / 2];
    valid_duration(est).then_some(est)
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
        let _ = i.set_option("length", 0.0f64);
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
        if read_raw_duration(m).is_some_and(|d| d > MAX_VOB_DUR_SEC) {
            return None;
        }
        if Instant::now() >= deadline {
            return None;
        }
        match m.wait_event(0.05) {
            Some(Ok(Event::FileLoaded)) => {
                if let Some(d) = read_duration(m) {
                    return Some(d);
                }
                if read_raw_duration(m).is_some_and(|d| d > MAX_VOB_DUR_SEC) {
                    return None;
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

fn resolve_probe_duration(m: &mut Mpv, path: &Path) -> Option<f64> {
    if let Some(d) = probe_with_session(m, path).filter(|d| valid_duration(*d)) {
        return Some(d);
    }
    chain_head_duration(m, path)
}

fn probe_with_session(m: &mut Mpv, path: &Path) -> Option<f64> {
    let src = path.to_str()?;
    drain_events(m);
    let _ = m.command("stop", &[]);
    drain_events(m);
    if m.command("loadfile", &[src, "replace"]).is_err() {
        crate::dvd_vob_log::dvd_seek_log(format!("mpv probe loadfile failed {}", path.display()));
        return None;
    }
    let started = Instant::now();
    let deadline = started + Duration::from_secs(PROBE_WAIT_SECS);
    let dur = wait_vob_duration(m, deadline);
    let _ = m.command("stop", &[]);
    drain_events(m);
    if dur.is_none() {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "mpv probe no duration {} (after {:.1}s)",
            path.display(),
            started.elapsed().as_secs_f64()
        ));
    }
    dur
}

fn store_probe_result(path: &Path, dur: Option<f64>) {
    if let Some(d) = dur.filter(|x| valid_duration(*x)) {
        crate::db::set_duration(path, d);
    }
}

/// Whole-file duration in seconds from libmpv (in-process cache + SQLite per path).
pub fn probe_vob_duration(path: &Path) -> Option<f64> {
    if !path.is_file() {
        return None;
    }
    let key = cache_key(path);
    if let Some(hit) = cache_get(&key) {
        return hit.filter(|d| valid_duration(*d));
    }
    let dur = PROBE_MPV.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = new_probe_mpv();
        }
        slot.as_mut().and_then(|m| resolve_probe_duration(m, path))
    });
    cache_set(key, dur);
    store_probe_result(path, dur);
    dur
}

pub(crate) fn clear_probe_cache_for_paths(paths: &[std::path::PathBuf]) {
    for p in paths {
        let key = cache_key(p);
        if let Ok(mut guard) = CACHE.lock() {
            if let Some(map) = guard.as_mut() {
                map.remove(&key);
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn clear_probe_cache() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(HashMap::new());
    }
    PROBE_MPV.with(|cell| *cell.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpv_probe_dvd9_chain_head_vob() {
        let vob = Path::new(
            "/Volumes/SanDisk/Torrents/Fritt.vilt.2006.DVD9/VIDEO_TS/VTS_01_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        clear_probe_cache();
        let d = probe_vob_duration(vob).expect("chain-head duration");
        assert!(
            d > 1000.0 && d < 1200.0,
            "expected ~1072s from sibling rate, got {d}"
        );
        assert!(d < 10_000.0, "must not return chained whole-title length");
    }

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
            started.elapsed() < Duration::from_secs(8),
            "probe took {:.1}s",
            started.elapsed().as_secs_f64()
        );
        let d2 = probe_vob_duration(vob).expect("cached");
        assert!((d - d2).abs() < 1e-3);
        assert!(
            started.elapsed() < Duration::from_secs(9),
            "cached probe should be instant"
        );
    }
}
