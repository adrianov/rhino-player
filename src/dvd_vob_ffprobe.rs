//! Per-`.vob` duration via `ffprobe` for the DVD unified timeline.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use crate::dvd_vob_timeline::MAX_VOB_DUR_SEC;

static CACHE: Mutex<Option<HashMap<String, f64>>> = Mutex::new(None);

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

fn parse_duration_stdout(raw: &str) -> Option<f64> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let d = s.parse::<f64>().ok()?;
    (d.is_finite() && d > 0.0 && d <= MAX_VOB_DUR_SEC).then_some(d)
}

/// Whole-file duration in seconds from `ffprobe`, with an in-process cache per canonical path.
pub fn probe_vob_duration(path: &Path) -> Option<f64> {
    if !path.is_file() {
        return None;
    }
    let key = cache_key(path);
    if let Some(hit) = cache_get(&key) {
        return hit.filter(|d| d.is_finite() && *d > 0.0);
    }
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(path)
        .output();
    let dur = match out {
        Ok(o) if o.status.success() => parse_duration_stdout(&String::from_utf8_lossy(&o.stdout)),
        Ok(o) => {
            crate::dvd_vob_log::dvd_seek_log(format!(
                "ffprobe failed {} status={} err={}",
                path.display(),
                o.status,
                String::from_utf8_lossy(&o.stderr).trim()
            ));
            None
        }
        Err(e) => {
            crate::dvd_vob_log::dvd_seek_log(format!("ffprobe spawn failed: {e}"));
            None
        }
    };
    cache_set(key, dur);
    dur
}

#[cfg(test)]
pub(crate) fn clear_probe_cache() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(HashMap::new());
    }
}
