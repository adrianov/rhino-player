// Neighbour (sibling) search for the continue screen — feature hub.
// See docs/features/33-continue-sibling-search.md. Split across:
//   sibling_search.rs          — NeighbourEntry, CatalogMem (snap/search/Lucky API), strip plan
//   lucky/                     — I'm Feeling Lucky picks (`recent_view::lucky`; maps from CatalogMem)
//   sibling_search_score.rs    — Jaccard trigrams (`#[path]`)
//   sibling_search_state.rs    — query / strip wiring (`#[path]`)
//   sibling_search_bind.rs     — card trash/remove API + hide (`#[path]`)
//   sibling_search_paint.rs    — paint-key compare (`#[path]` from state)
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

/// One neighbour path plus cached name and openability (hollow preflight is lazy).
#[derive(Clone, Debug, PartialEq, Eq)]
struct NeighbourEntry {
    path: PathBuf,
    name_lower: String,
    /// `None` until missing/hollow preflight runs; search ranks names first.
    openable: Cell<Option<bool>>,
}

impl NeighbourEntry {
    fn pending(path: PathBuf) -> Self {
        let name_lower = file_name_lower(&path);
        Self {
            path,
            name_lower,
            openable: Cell::new(None),
        }
    }

    #[cfg(test)]
    fn known(path: PathBuf, openable: bool) -> Self {
        let name_lower = file_name_lower(&path);
        Self {
            path,
            name_lower,
            openable: Cell::new(Some(openable)),
        }
    }

    fn is_openable(&self) -> bool {
        if let Some(v) = self.openable.get() {
            return v;
        }
        let v = classify_openable(&self.path);
        self.openable.set(Some(v));
        v
    }

    fn set_openable(&self, openable: bool) {
        self.openable.set(Some(openable));
    }

    fn known_unopenable(&self) -> bool {
        self.openable.get() == Some(false)
    }
}

/// Session catalog + progress for search / Lucky (feature 33).
/// Sole owner of SQLite load/refresh; selection, Lucky, and paint keys go through this API.
type ProgressMaps = (HashMap<String, f64>, HashMap<String, f64>);
type PaintKey = Vec<(PathBuf, u64, u64)>;

struct CatalogMem {
    index: RefCell<Vec<NeighbourEntry>>,
    scanned: Cell<bool>,
    progress: RefCell<ProgressMaps>,
}

impl CatalogMem {
    fn new() -> Self {
        Self {
            index: RefCell::default(),
            scanned: Cell::new(false),
            progress: RefCell::default(),
        }
    }

    fn ensure(&self) {
        if self.scanned.get() {
            return;
        }
        let paths = crate::db::list_file_paths();
        eprintln!(
            "[rhino] search: index n={} (catalog snap, in-memory filter)",
            paths.len()
        );
        *self.index.borrow_mut() = paths.into_iter().map(NeighbourEntry::pending).collect();
        *self.progress.borrow_mut() = (
            crate::db::load_time_pos_map(),
            crate::db::load_duration_map(),
        );
        self.scanned.set(true);
    }

    fn refresh_progress(&self) {
        *self.progress.borrow_mut() = (
            crate::db::load_time_pos_map(),
            crate::db::load_duration_map(),
        );
    }

    fn ready(&self) -> bool {
        self.scanned.get()
    }

    fn index(&self) -> std::cell::Ref<'_, Vec<NeighbourEntry>> {
        self.index.borrow()
    }

    fn index_mut(&self) -> std::cell::RefMut<'_, Vec<NeighbourEntry>> {
        self.index.borrow_mut()
    }

    /// Name-search hits from the in-memory catalog (no SQLite).
    fn name_hits(&self, q: &str) -> (Vec<PathBuf>, bool) {
        let index = self.index.borrow();
        let (tpos, _) = &*self.progress.borrow();
        capped_name_hits(&index, q, tpos)
    }

    /// Retarget Lucky titles then return the openable shown handful.
    fn lucky_hits(&self, lucky: &lucky::LuckySession) -> Option<Vec<PathBuf>> {
        let index = self.index.borrow();
        let (tpos, durs) = &*self.progress.borrow();
        lucky.retarget(&index, tpos, durs);
        lucky.strip_hits(&index)
    }

    fn lucky_roll(&self, lucky: &lucky::LuckySession, max: usize) {
        self.ensure();
        self.refresh_progress();
        let index = self.index.borrow();
        let (tpos, durs) = &*self.progress.borrow();
        lucky.roll(&index, max, tpos, durs);
    }

    fn lucky_refill(&self, lucky: &lucky::LuckySession, gone: &Path) -> bool {
        let index = self.index.borrow();
        let (tpos, durs) = &*self.progress.borrow();
        lucky.refill_slot(gone, &index, tpos, durs)
    }

    fn lucky_warm(&self, lucky: &lucky::LuckySession, paths: &mut Vec<PathBuf>) {
        lucky.append_warm(paths, &self.index.borrow());
    }

    /// Paint skip-key from cached resume/duration (no SQLite).
    fn paint_key(&self, paths: &[PathBuf]) -> PaintKey {
        let (tpos, durs) = &*self.progress.borrow();
        paths
            .iter()
            .map(|p| {
                let (resume, dur) = crate::playback_entity::card_resume_duration(p, durs, tpos);
                (p.clone(), resume.to_bits(), dur.to_bits())
            })
            .collect()
    }
}

/// Fill `index` from `build` at most once (session catalog index).
#[cfg(test)]
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
use std::collections::HashSet;
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

/// Score, in-progress flag, lowercased file name, index into the neighbour list.
type ScoredHit = (f64, bool, String, usize);

/// Name hits among openable index entries. In-memory only: no canonicalize or entity resolve.
#[cfg(test)]
fn present_name_hits(entries: &[NeighbourEntry], q: &str) -> Vec<PathBuf> {
    let mut scored = score_name_hits(entries, q, &HashMap::new());
    sort_scored_hits(&mut scored);
    scored.retain(|h| entries[h.3].is_openable());
    hit_paths(entries, scored)
}

/// Same ranking as [present_name_hits], but only the strip cap is cloned (wide one-letter queries).
fn capped_name_hits(
    entries: &[NeighbourEntry],
    q: &str,
    tpos: &HashMap<String, f64>,
) -> (Vec<PathBuf>, bool) {
    let mut scored = score_name_hits(entries, q, tpos);
    sort_scored_hits(&mut scored);
    take_openable_hits(entries, scored)
}

#[cfg(test)]
fn hit_paths(entries: &[NeighbourEntry], scored: Vec<ScoredHit>) -> Vec<PathBuf> {
    scored
        .into_iter()
        .map(|h| entries[h.3].path.clone())
        .collect()
}

/// Rank every name that is not already known-unopenable; preflight runs in [take_openable_hits].
fn score_name_hits(
    entries: &[NeighbourEntry],
    q: &str,
    tpos: &HashMap<String, f64>,
) -> Vec<ScoredHit> {
    let q_tri = query_trigrams(q);
    let progress = progress_name_keys(tpos);
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.known_unopenable())
        .filter_map(|(i, e)| score_name_hit(e, i, q, &q_tri, &progress))
        .collect()
}

fn take_openable_hits(entries: &[NeighbourEntry], scored: Vec<ScoredHit>) -> (Vec<PathBuf>, bool) {
    let mut hits = Vec::new();
    let mut capped = false;
    for h in scored {
        if !entries[h.3].is_openable() {
            continue;
        }
        if hits.len() < SEARCH_MAX_HITS {
            hits.push(entries[h.3].path.clone());
        } else {
            capped = true;
            break;
        }
    }
    (hits, capped)
}

fn score_name_hit(
    e: &NeighbourEntry,
    idx: usize,
    q: &str,
    q_tri: &HashSet<(char, char, char)>,
    progress: &HashSet<String>,
) -> Option<ScoredHit> {
    let score = name_match_score(&e.name_lower, q, q_tri)?;
    let started = name_in_progress(&e.path, &e.name_lower, progress);
    Some((score, started, e.name_lower.clone(), idx))
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
            .map(|i| NeighbourEntry::known(PathBuf::from(format!("/store/pick{i}.mkv")), true))
            .collect();
        let (hits, capped) = capped_name_hits(&entries, "pick", &std::collections::HashMap::new());
        assert!(capped);
        assert_eq!(hits.len(), SEARCH_MAX_HITS);
    }
}
