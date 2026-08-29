// Neighbour (sibling) search for the continue screen — feature hub.
// See docs/features/33-continue-sibling-search.md. Split across:
//   sibling_search.rs          — BFS scan, hit filter, strip plan, tests
//   sibling_search_score.rs    — Jaccard trigrams (`#[path]`)
//   sibling_search_state.rs    — query / index / paint (`#[path]`)
//   sibling_search_input.rs    — debounce / commit (`#[path]` from state)
//   sibling_search_widgets.rs  — search-row widgets
// NOTE: include!'d into `recent_view`; shares its imports (glib, Rc, RefCell, Path, Duration).

include!("sibling_search_widgets.rs");
#[path = "sibling_search_score.rs"]
mod sibling_search_score;
use sibling_search_score::{name_match_score, query_trigrams};
#[path = "sibling_search_state.rs"]
mod sibling_search_state;
pub(crate) use sibling_search_state::*;


fn take_capped(mut hits: Vec<PathBuf>) -> (Vec<PathBuf>, bool) {
    let capped = hits.len() > SEARCH_MAX_HITS;
    hits.truncate(SEARCH_MAX_HITS);
    (hits, capped)
}

/// One neighbour path plus openability learned when the session index was built (or refreshed).
#[derive(Clone, Debug, PartialEq, Eq)]
struct NeighbourEntry {
    path: PathBuf,
    openable: bool,
}

/// Fill `index` from `build` at most once (session neighbour scan).
fn index_fill_once(
    scanned: &Cell<bool>,
    index: &RefCell<Vec<NeighbourEntry>>,
    build: impl FnOnce() -> Vec<NeighbourEntry>,
) -> Vec<NeighbourEntry> {
    if !scanned.get() {
        *index.borrow_mut() = build();
        scanned.set(true);
    }
    index.borrow().clone()
}

/// True when open preflight would allow a load (no missing / empty / hollow stub).
fn classify_openable(path: &Path) -> bool {
    crate::media_open_fail::preflight_user_message(path).is_none()
}

use std::cell::Cell;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

/// Result cards shown at most; a huge library folder must not flood the strip.
pub(crate) const SEARCH_MAX_HITS: usize = 40;

/// What a continue-strip repaint should draw given the active neighbour query.
pub(crate) struct StripPlan {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) kind: StripKind,
    pub(crate) searching: bool,
}

/// Resolve strip contents: neighbour hits while a query is active, else the fallback list.
pub(crate) fn strip_plan(
    search: Option<&SiblingSearchState>,
    fallback: Vec<PathBuf>,
) -> StripPlan {
    match search.and_then(|s| s.current_hits()) {
        Some(paths) => StripPlan {
            paths,
            kind: StripKind::NeighbourHits,
            searching: true,
        },
        None => StripPlan {
            paths: fallback,
            kind: StripKind::ContinueList,
            searching: false,
        },
    }
}

/// Filesystem root (`/` / drive root) — never scanned; its children are not sibling dirs.
fn is_fs_root(p: &Path) -> bool {
    p.parent().is_none()
}

/// Parent dirs of catalog paths, plus each parent’s sibling dirs when the grandparent is not root.
/// BFS over the dir queue; each dir is listed once (non-recursive video files).
fn scan_sibling_universe(seeds: &[PathBuf]) -> Vec<PathBuf> {
    let mut queue = VecDeque::new();
    let mut seen_dirs = HashSet::new();
    for seed in seeds {
        let Some(dir) = seed.parent() else {
            continue;
        };
        enqueue_scan_dir(&mut queue, &mut seen_dirs, dir);
        enqueue_sibling_dirs(&mut queue, &mut seen_dirs, dir);
    }
    let mut files = Vec::new();
    let mut seen_files = HashSet::new();
    while let Some(dir) = queue.pop_front() {
        let Some(videos) = crate::video_ext::list_videos_in_dir(&dir) else {
            continue;
        };
        for v in videos {
            if seen_files.insert(v.clone()) {
                files.push(v);
            }
        }
    }
    sort_neighbours(&mut files);
    files
}

fn enqueue_sibling_dirs(
    queue: &mut VecDeque<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    dir: &Path,
) {
    let Some(grand) = dir.parent() else {
        return;
    };
    if is_fs_root(grand) {
        return;
    }
    let Ok(rd) = std::fs::read_dir(grand) else {
        return;
    };
    for ent in rd.filter_map(Result::ok) {
        let Ok(ft) = ent.file_type() else {
            continue;
        };
        if ft.is_dir() {
            enqueue_scan_dir(queue, seen, &ent.path());
        }
    }
}

fn enqueue_scan_dir(queue: &mut VecDeque<PathBuf>, seen: &mut HashSet<PathBuf>, dir: &Path) {
    if is_fs_root(dir) {
        return;
    }
    let owned = dir.to_path_buf();
    if seen.insert(owned.clone()) {
        queue.push_back(owned);
    }
}

/// Build the session neighbour index from the files catalog (once per [SiblingSearchState]).
fn build_neighbour_index() -> Vec<NeighbourEntry> {
    let files = scan_sibling_universe(&crate::db::list_file_paths());
    crate::db::ensure_files(&files);
    files
        .into_iter()
        .map(|path| NeighbourEntry {
            openable: classify_openable(&path),
            path,
        })
        .collect()
}

/// Name hits among openable index entries, ranked by token trigram Jaccard (feature 33).
fn present_name_hits(entries: &[NeighbourEntry], q: &str) -> Vec<PathBuf> {
    let q_tri = query_trigrams(q);
    let mut scored: Vec<(f64, PathBuf)> = entries
        .iter()
        .filter(|e| e.openable)
        .filter_map(|e| {
            let name = file_name_lower(&e.path);
            name_match_score(&name, q, &q_tri).map(|s| (s, e.path.clone()))
        })
        .collect();
    sort_scored_hits(&mut scored);
    scored.into_iter().map(|(_, p)| p).collect()
}

fn sort_scored_hits(scored: &mut [(f64, PathBuf)]) {
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                lexical_sort::natural_lexical_cmp(&file_name_lower(&a.1), &file_name_lower(&b.1))
            })
    });
}

fn file_name_lower(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn sort_neighbours(v: &mut [PathBuf]) {
    v.sort_by(|a, b| lexical_sort::natural_lexical_cmp(&file_name_lower(a), &file_name_lower(b)));
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("sibling_search_tests.rs");
}
