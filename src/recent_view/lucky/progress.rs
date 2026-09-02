// Resume / duration from the loaded store maps — listing path or file name, no disk.

use std::collections::HashMap;
use std::path::Path;

pub(super) struct ProgressLookup<'a> {
    tpos: &'a HashMap<String, f64>,
    durs: &'a HashMap<String, f64>,
    tpos_name: HashMap<String, f64>,
    durs_name: HashMap<String, f64>,
}

impl<'a> ProgressLookup<'a> {
    pub(super) fn new(tpos: &'a HashMap<String, f64>, durs: &'a HashMap<String, f64>) -> Self {
        Self {
            tpos,
            durs,
            tpos_name: name_map(tpos, true),
            durs_name: name_map(durs, false),
        }
    }

    pub(super) fn is_watching(&self, path: &Path) -> bool {
        let resume = store_get(path, self.tpos, &self.tpos_name);
        if !(resume.is_finite() && resume > 0.0) {
            return false;
        }
        let dur = store_get(path, self.durs, &self.durs_name);
        dur <= 0.0 || !crate::media_probe::past_done_mark(resume, dur)
    }
}

fn store_get(path: &Path, by_path: &HashMap<String, f64>, by_name: &HashMap<String, f64>) -> f64 {
    path.to_str()
        .and_then(|s| by_path.get(s).copied())
        .or_else(|| {
            path.file_name()
                .and_then(|n| by_name.get(&n.to_string_lossy().to_lowercase()).copied())
        })
        .unwrap_or(0.0)
}

fn name_map(map: &HashMap<String, f64>, need_progress: bool) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    for (k, &v) in map {
        if !v.is_finite() || (need_progress && v <= 0.0) {
            continue;
        }
        if let Some(n) = Path::new(k).file_name() {
            out.insert(n.to_string_lossy().to_lowercase(), v);
        }
    }
    out
}
