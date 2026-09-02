// Neighbour (sibling) search for the continue screen — feature hub.
// See docs/features/33-continue-sibling-search.md. Split across:
//   sibling_search.rs          — BFS scan, hit filter, strip plan, tests
//   lucky/                     — I'm Feeling Lucky owner (`recent_view::lucky`)
//   sibling_search_score.rs    — Jaccard trigrams (`#[path]`)
//   sibling_search_state.rs    — query / index / paint / lucky dismiss (`#[path]`)
//   sibling_search_bind.rs     — card trash/remove API + hide (`#[path]`)
//   sibling_search_paint.rs    — neighbour paint key (`#[path]` from state)
//   sibling_search_input.rs    — debounce / commit / lucky click (`#[path]` from state)
//   sibling_search_widgets.rs  — search-row widgets + I'm Feeling Lucky
// NOTE: include!'d into `recent_view`; shares its imports (glib, Rc, RefCell, Path, Duration).

include!("sibling_search_widgets.rs");
#[path = "sibling_search_score.rs"]
mod sibling_search_score;
use sibling_search_score::{name_match_score, query_trigrams};
#[path = "sibling_search_state.rs"]
mod sibling_search_state;
pub(crate) use sibling_search_state::*;
#[path = "sibling_search_bind.rs"]
mod sibling_search_bind;
pub(crate) use sibling_search_bind::*;

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
) {
    if !scanned.get() {
        *index.borrow_mut() = build();
        scanned.set(true);
    }
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

/// Which population a strip paint carries (affects hover chrome: Remove vs Trash-only).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StripKind {
    /// Watch-later entries plus the Open Video tile (default).
    ContinueList,
    /// Name-search hits (Trash only).
    NeighbourHits,
    /// I'm Feeling Lucky (Trash + Remove; feature 33).
    Lucky,
}

impl StripKind {
    pub fn shows_remove(self) -> bool {
        !matches!(self, Self::NeighbourHits)
    }

    pub fn hits_strip(self) -> bool {
        matches!(self, Self::NeighbourHits | Self::Lucky)
    }
}

/// What a continue-strip repaint should draw given the active neighbour query.
pub(crate) struct StripPlan {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) kind: StripKind,
    pub(crate) searching: bool,
}

/// Resolve strip contents: neighbour hits while a query is active, else the fallback list.
pub(crate) fn strip_plan(search: Option<&SiblingSearchState>, fallback: Vec<PathBuf>) -> StripPlan {
    if let Some(s) = search {
        if let Some(paths) = s.current_hits() {
            return StripPlan {
                paths,
                kind: s.hits_kind(),
                searching: true,
            };
        }
    }
    StripPlan {
        paths: fallback,
        kind: StripKind::ContinueList,
        searching: false,
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

fn enqueue_sibling_dirs(queue: &mut VecDeque<PathBuf>, seen: &mut HashSet<PathBuf>, dir: &Path) {
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

/// Score, in-progress flag, lowercased file name, index into the neighbour list.
type ScoredHit = (f64, bool, String, usize);

/// Name hits among openable index entries. In-memory only: no canonicalize or entity resolve.
#[cfg(test)]
fn present_name_hits(entries: &[NeighbourEntry], q: &str) -> Vec<PathBuf> {
    let mut scored = score_openable_hits(entries, q);
    sort_scored_hits(&mut scored);
    hit_paths(entries, scored)
}

/// Same ranking as [present_name_hits], but only the strip cap is cloned (wide one-letter queries).
fn capped_name_hits(entries: &[NeighbourEntry], q: &str) -> (Vec<PathBuf>, bool) {
    let mut scored = score_openable_hits(entries, q);
    let capped = scored.len() > SEARCH_MAX_HITS;
    if capped {
        scored.select_nth_unstable_by(SEARCH_MAX_HITS, hit_ord);
        scored.truncate(SEARCH_MAX_HITS);
    }
    sort_scored_hits(&mut scored);
    (hit_paths(entries, scored), capped)
}

fn hit_paths(entries: &[NeighbourEntry], scored: Vec<ScoredHit>) -> Vec<PathBuf> {
    scored
        .into_iter()
        .map(|h| entries[h.3].path.clone())
        .collect()
}

fn score_openable_hits(entries: &[NeighbourEntry], q: &str) -> Vec<ScoredHit> {
    let q_tri = query_trigrams(q);
    let progress = progress_name_keys(&crate::db::load_time_pos_map());
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.openable)
        .filter_map(|(i, e)| score_openable_hit(e, i, q, &q_tri, &progress))
        .collect()
}

fn score_openable_hit(
    e: &NeighbourEntry,
    idx: usize,
    q: &str,
    q_tri: &HashSet<(char, char, char)>,
    progress: &HashSet<String>,
) -> Option<ScoredHit> {
    let name = file_name_lower(&e.path);
    let score = name_match_score(&name, q, q_tri)?;
    let started = name_in_progress(&e.path, &name, progress);
    Some((score, started, name, idx))
}

/// Resume keys and their lowercased file names — lookup only, no disk.
fn progress_name_keys(tpos: &HashMap<String, f64>) -> HashSet<String> {
    let mut keys = HashSet::new();
    for (k, t) in tpos {
        if !t.is_finite() || *t <= 0.0 {
            continue;
        }
        keys.insert(k.clone());
        if let Some(n) = Path::new(k).file_name() {
            keys.insert(n.to_string_lossy().to_lowercase());
        }
    }
    keys
}

fn name_in_progress(path: &Path, name_lower: &str, keys: &HashSet<String>) -> bool {
    path.to_str().is_some_and(|s| keys.contains(s)) || keys.contains(name_lower)
}

/// Score ↓, then in-progress before unstarted, then natural file name.
fn hit_ord(a: &ScoredHit, b: &ScoredHit) -> std::cmp::Ordering {
    b.0.partial_cmp(&a.0)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| b.1.cmp(&a.1))
        .then_with(|| lexical_sort::natural_lexical_cmp(&a.2, &b.2))
}

fn sort_scored_hits(scored: &mut [ScoredHit]) {
    scored.sort_by(hit_ord);
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

#[cfg(test)]
mod rank_tests {
    use super::*;

    #[test]
    fn equal_score_prefers_in_progress_over_unstarted() {
        let mut scored = [
            scored_hit(0.5, false, "/store/ep05.mkv"),
            scored_hit(0.5, true, "/store/ep07.mkv"),
            scored_hit(0.5, false, "/store/ep06.mkv"),
        ];
        sort_scored_hits(&mut scored);
        assert_eq!(scored[0].2, "ep07.mkv");
        assert!(scored[0].1);
    }

    fn scored_hit(score: f64, started: bool, path: &str) -> ScoredHit {
        (score, started, file_name_lower(Path::new(path)), 0)
    }

    #[test]
    fn progress_uses_store_keys_without_disk() {
        let mut tpos = HashMap::new();
        tpos.insert("/store/watch.mkv".into(), 12.0);
        tpos.insert("/store/zero.mkv".into(), 0.0);
        let keys = progress_name_keys(&tpos);
        assert!(name_in_progress(
            Path::new("/other/watch.mkv"),
            "watch.mkv",
            &keys
        ));
        assert!(!name_in_progress(
            Path::new("/store/zero.mkv"),
            "zero.mkv",
            &keys
        ));
    }

    #[test]
    fn capped_hits_keep_only_the_strip_limit() {
        let entries: Vec<_> = (0..SEARCH_MAX_HITS + 5)
            .map(|i| NeighbourEntry {
                path: PathBuf::from(format!("/store/pick{i}.mkv")),
                openable: true,
            })
            .collect();
        let (hits, capped) = capped_name_hits(&entries, "pick");
        assert!(capped);
        assert_eq!(hits.len(), SEARCH_MAX_HITS);
    }
}
