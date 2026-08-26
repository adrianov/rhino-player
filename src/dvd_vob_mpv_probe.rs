//! Per-`.vob` duration via a reused headless libmpv instance for the DVD unified timeline.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use libmpv2::Mpv;

use crate::dvd_vob_timeline::MAX_VOB_DUR_SEC;

static CACHE: Mutex<Option<HashMap<String, f64>>> = Mutex::new(None);

thread_local! {
    static PROBE_MPV: RefCell<Option<Mpv>> = const { RefCell::new(None) };
}

include!("dvd_vob_mpv_probe/session.rs");

fn cache_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// Probe through the reused thread-local headless mpv instance.
fn resolve_with_reused_mpv(path: &Path) -> Option<f64> {
    PROBE_MPV.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = new_probe_mpv();
        }
        slot.as_mut().and_then(|m| resolve_probe_duration(m, path))
    })
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

/// Whole-file duration in seconds from libmpv (in-process cache + SQLite per path).
pub fn probe_vob_duration(path: &Path) -> Option<f64> {
    if !path.is_file() {
        return None;
    }
    let key = cache_key(path);
    if let Some(hit) = cache_get(&key) {
        return hit.filter(|d| valid_duration(*d));
    }
    let dur = resolve_with_reused_mpv(path);
    cache_set(key, dur);
    store_probe_result(path, dur);
    dur
}

fn store_probe_result(path: &Path, dur: Option<f64>) {
    if let Some(d) = dur.filter(|x| valid_duration(*x)) {
        crate::db::set_duration(path, d);
    }
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
    use std::time::{Duration, Instant};

    #[test]
    fn mpv_probe_dvd9_chain_head_vob() {
        let vob = Path::new("/Volumes/SanDisk/Torrents/Fritt.vilt.2006.DVD9/VIDEO_TS/VTS_01_1.VOB");
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
