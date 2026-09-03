// Collapse neighbour paths to one lucky title per series or standalone file.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use lexical_sort::natural_lexical_cmp;
use regex::Regex;

use super::progress::ProgressLookup;
use super::NeighbourEntry;

pub(super) fn lucky_titles(
    entries: &[NeighbourEntry],
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) -> Vec<(String, PathBuf)> {
    titles_from_groups(&group_index(entries), &openable_set(entries), tpos, durs)
}

/// Title id → listing paths (built once per lucky session).
pub(super) fn group_index(entries: &[NeighbourEntry]) -> HashMap<String, Vec<PathBuf>> {
    let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for e in entries {
        groups
            .entry(title_id(&e.path))
            .or_default()
            .push(e.path.clone());
    }
    groups
}

pub(super) fn openable_set(entries: &[NeighbourEntry]) -> HashSet<&Path> {
    // Known-openable or not yet checked — never force hollow preflight on the whole catalog.
    // Strip paint / keep_openable still preflight only paths placed on the strip.
    entries
        .iter()
        .filter(|e| !e.known_unopenable())
        .map(|e| e.path.as_path())
        .collect()
}

pub(super) fn titles_from_groups(
    groups: &HashMap<String, Vec<PathBuf>>,
    open: &HashSet<&Path>,
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) -> Vec<(String, PathBuf)> {
    let store = ProgressLookup::new(tpos, durs);
    groups
        .iter()
        .filter_map(|(id, paths)| playable_pick(paths, open, &store).map(|p| (id.clone(), p)))
        .collect()
}

/// Rewrite shown/reserved paths to each title's current continue-or-first episode.
pub(super) fn retarget_lists(
    shown: &mut Option<Vec<PathBuf>>,
    next: &mut Option<Vec<PathBuf>>,
    groups: &HashMap<String, Vec<PathBuf>>,
    open: &HashSet<&Path>,
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) {
    let store = ProgressLookup::new(tpos, durs);
    rewrite_list(shown, groups, open, &store);
    rewrite_list(next, groups, open, &store);
}

fn rewrite_list(
    paths: &mut Option<Vec<PathBuf>>,
    groups: &HashMap<String, Vec<PathBuf>>,
    open: &HashSet<&Path>,
    store: &ProgressLookup<'_>,
) {
    if let Some(paths) = paths.as_mut() {
        rewrite_slice(paths, groups, open, store);
    }
}

fn rewrite_slice(
    paths: &mut [PathBuf],
    groups: &HashMap<String, Vec<PathBuf>>,
    open: &HashSet<&Path>,
    store: &ProgressLookup<'_>,
) {
    for p in paths {
        if let Some(fresh) = groups
            .get(&title_id(p))
            .and_then(|members| playable_pick(members, open, store))
        {
            *p = fresh;
        }
    }
}

fn playable_pick(
    paths: &[PathBuf],
    open: &HashSet<&Path>,
    store: &ProgressLookup<'_>,
) -> Option<PathBuf> {
    let playable: Vec<&PathBuf> = paths
        .iter()
        .filter(|p| open.contains(p.as_path()))
        .collect();
    (!playable.is_empty()).then(|| pick_title(&playable, store))
}

/// Rewrite each path to that title's current continue-or-first episode.
#[cfg(test)]
pub(super) fn retarget_paths(
    paths: &mut [PathBuf],
    index: &[NeighbourEntry],
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) {
    let groups = group_index(index);
    rewrite_slice(
        paths,
        &groups,
        &openable_set(index),
        &ProgressLookup::new(tpos, durs),
    );
}

fn title_id(path: &Path) -> String {
    folder_title_id(path)
        .or_else(|| episode_title_id(path))
        .unwrap_or_else(|| format!("f:{}", path.to_string_lossy()))
}

fn folder_title_id(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    let pname = parent.file_name()?.to_str()?;
    let stem = grouped_folder_stem(pname, path)?;
    let root = parent.parent().unwrap_or(parent);
    Some(format!("s:{}|{stem}", root.to_string_lossy()))
}

fn grouped_folder_stem(pname: &str, path: &Path) -> Option<String> {
    if crate::sibling_advance::folder_looks_seasonal(pname) {
        return Some(crate::sibling_advance::folder_series_stem(pname));
    }
    episode_cut(pname)?;
    file_series_stem(pname).or_else(|| {
        path.file_name()
            .and_then(|n| n.to_str())
            .and_then(file_series_stem)
    })
}

fn episode_title_id(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    let fname = path.file_name()?.to_str()?;
    let stem = file_series_stem(fname)?;
    Some(format!("s:{}|{stem}", parent.to_string_lossy()))
}

/// Start of the first episode / season-episode marker in a file or folder name.
fn episode_cut(name: &str) -> Option<usize> {
    episode_marker().find(name).map(|m| m.start())
}

fn episode_marker() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // SxxExx / NxNN; "episode 7"; Russian "серия 7" / "11 сер." / "(07 сер.)".
        // `сер\b` needs a word edge so it does not fire inside `сериал`.
        Regex::new(concat!(
            r"(?i)(?:\bS\d{1,2}[\s._-]*E\d{1,3}\b|\b\d{1,2}x\d{2}\b|",
            r"\b(?:episode|ep\.?|серия|серии|сер\b)\s*\d+|",
            r"(?:\(|\b)\d{1,3}[\s._-]*(?:episode|ep\.?|серия|серии|сер\b)",
            r")",
        ))
        .expect("episode marker")
    })
}

fn file_series_stem(name: &str) -> Option<String> {
    let cut = episode_cut(name)?;
    let raw = name[..cut].replace(['.', '_', '-'], " ");
    let stem = raw
        .trim_end_matches(['(', '[', '—', '–', '-', ' '])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    (!stem.is_empty()).then_some(stem)
}

fn pick_title(paths: &[&PathBuf], store: &ProgressLookup<'_>) -> PathBuf {
    paths
        .iter()
        .copied()
        .filter(|p| store.is_watching(p))
        .max_by(|a, b| path_ord(a, b))
        .or_else(|| paths.iter().copied().min_by(|a, b| path_ord(a, b)))
        .expect("non-empty title group")
        .clone()
}

fn path_ord(a: &Path, b: &Path) -> std::cmp::Ordering {
    natural_lexical_cmp(&a.to_string_lossy(), &b.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::file_series_stem;

    #[test]
    fn stem_from_russian_ser_paren() {
        assert_eq!(
            file_series_stem("Экспроприатор (11 сер.).mkv"),
            Some("экспроприатор".into())
        );
        assert_eq!(
            file_series_stem("Экспроприатор (07 сер.).mkv"),
            Some("экспроприатор".into())
        );
    }

    #[test]
    fn stem_from_number_then_episode_word() {
        assert_eq!(file_series_stem("Show (11 серия).mkv"), Some("show".into()));
        assert_eq!(file_series_stem("Show.11.сер.mkv"), Some("show".into()));
        assert_eq!(file_series_stem("Show S01.E02.mkv"), Some("show".into()));
    }

    #[test]
    fn stem_skips_names_without_episode_marker() {
        assert!(file_series_stem("Экспроприатор.mkv").is_none());
        assert!(file_series_stem("Лучший сериал.mkv").is_none());
    }
}
