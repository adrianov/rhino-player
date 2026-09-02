// Collapse neighbour paths to one lucky title per series or standalone file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use lexical_sort::natural_lexical_cmp;
use regex::Regex;

use super::NeighbourEntry;

pub(super) fn lucky_titles(
    entries: &[NeighbourEntry],
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) -> Vec<(String, PathBuf)> {
    let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for e in entries.iter().filter(|e| e.openable) {
        groups
            .entry(title_id(&e.path))
            .or_default()
            .push(e.path.clone());
    }
    groups
        .into_iter()
        .map(|(id, paths)| (id, pick_title(&paths, tpos, durs)))
        .collect()
}

/// Rewrite each path to that title's current continue-or-first episode.
pub(super) fn retarget_paths(
    paths: &mut [PathBuf],
    index: &[NeighbourEntry],
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) {
    let by_id: HashMap<_, _> = lucky_titles(index, tpos, durs).into_iter().collect();
    for p in paths {
        if let Some(fresh) = by_id.get(&title_id(p)) {
            *p = fresh.clone();
        }
    }
}

fn title_id(path: &Path) -> String {
    seasonal_title_id(path)
        .or_else(|| episode_title_id(path))
        .unwrap_or_else(|| format!("f:{}", path.to_string_lossy()))
}

fn seasonal_title_id(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    let pname = parent.file_name()?.to_str()?;
    if !crate::sibling_advance::folder_looks_seasonal(pname) {
        return None;
    }
    let stem = crate::sibling_advance::folder_series_stem(pname);
    let root = parent.parent().unwrap_or(parent);
    Some(format!("s:{}|{stem}", root.to_string_lossy()))
}

fn episode_title_id(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    let fname = path.file_name()?.to_str()?;
    let stem = file_series_stem(fname)?;
    Some(format!("s:{}|{stem}", parent.to_string_lossy()))
}

fn episode_cut(name: &str) -> Option<usize> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:\bS\d{1,2}E\d{1,3}\b|\b\d{1,2}x\d{2}\b|\b(?:episode|ep\.?|серия)\s*\d+)",
        )
        .expect("episode marker")
    });
    re.find(name).map(|m| m.start())
}

fn file_series_stem(name: &str) -> Option<String> {
    let cut = episode_cut(name)?;
    let raw = name[..cut].replace(['.', '_', '-'], " ");
    Some(
        raw.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase(),
    )
}

fn pick_title(
    paths: &[PathBuf],
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) -> PathBuf {
    watching_latest(paths, tpos, durs).unwrap_or_else(|| first_path(paths))
}

fn watching_latest(
    paths: &[PathBuf],
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) -> Option<PathBuf> {
    paths
        .iter()
        .filter(|p| is_watching(p, tpos, durs))
        .max_by(|a, b| path_ord(a, b))
        .cloned()
}

fn first_path(paths: &[PathBuf]) -> PathBuf {
    paths
        .iter()
        .min_by(|a, b| path_ord(a, b))
        .cloned()
        .unwrap_or_else(|| paths[0].clone())
}

fn path_ord(a: &Path, b: &Path) -> std::cmp::Ordering {
    natural_lexical_cmp(&a.to_string_lossy(), &b.to_string_lossy())
}

fn is_watching(path: &Path, tpos: &HashMap<String, f64>, durs: &HashMap<String, f64>) -> bool {
    let (resume, dur) = crate::playback_entity::card_resume_duration(path, durs, tpos);
    if !(resume.is_finite() && resume > 0.0) {
        return false;
    }
    dur <= 0.0 || !crate::media_probe::past_done_mark(resume, dur)
}
