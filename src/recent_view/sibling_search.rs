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

use std::cell::Cell;
use std::collections::HashSet;
use std::path::PathBuf;

/// Session catalog + progress for search / Lucky (feature 33).
/// Sole owner of SQLite load/refresh; selection, Lucky, and paint keys go through this API.
type ProgressMaps = (HashMap<String, f64>, HashMap<String, f64>);
type PaintKey = Vec<(PathBuf, u64, u64)>;

struct CatalogMem {
    index: RefCell<Vec<NeighbourEntry>>,
    scanned: Cell<bool>,
    progress: RefCell<ProgressMaps>,
    /// Resume path/name keys derived from [progress]; rebuilt only when progress reloads.
    progress_keys: RefCell<HashSet<String>>,
}

impl CatalogMem {
    fn new() -> Self {
        Self {
            index: RefCell::default(),
            scanned: Cell::new(false),
            progress: RefCell::default(),
            progress_keys: RefCell::default(),
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
        self.store_progress(
            crate::db::load_time_pos_map(),
            crate::db::load_duration_map(),
        );
        self.scanned.set(true);
    }

    fn refresh_progress(&self) {
        self.store_progress(
            crate::db::load_time_pos_map(),
            crate::db::load_duration_map(),
        );
    }

    fn store_progress(&self, tpos: HashMap<String, f64>, durs: HashMap<String, f64>) {
        *self.progress_keys.borrow_mut() = progress_name_keys(&tpos);
        *self.progress.borrow_mut() = (tpos, durs);
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
        let keys = self.progress_keys.borrow();
        capped_name_hits(&index, q, &keys)
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

/// Score, in-progress flag, index into the neighbour list (no per-hit name clone).
type ScoredHit = (f64, bool, usize);

/// How many ranked name matches to preflight per round while filling the strip.
const RANK_BATCH: usize = SEARCH_MAX_HITS;

/// Name hits among openable index entries. In-memory only: no canonicalize or entity resolve.
#[cfg(test)]
fn present_name_hits(entries: &[NeighbourEntry], q: &str) -> Vec<PathBuf> {
    let mut scored = score_name_hits(entries, q, &HashSet::new());
    sort_scored_hits(entries, &mut scored);
    scored.retain(|h| entries[h.2].is_openable());
    hit_paths(entries, scored)
}

/// Rank name matches in batches; preflight while filling strip slots (feature 33).
/// `capped` means at least one more **playable** hit exists past the strip limit.
fn capped_name_hits(
    entries: &[NeighbourEntry],
    q: &str,
    progress: &HashSet<String>,
) -> (Vec<PathBuf>, bool) {
    let mut pool = collect_name_hits(entries, q, progress);
    let mut hits = Vec::with_capacity(SEARCH_MAX_HITS.min(pool.len()));
    while hits.len() < SEARCH_MAX_HITS && !pool.is_empty() {
        let n = pool.len().min(RANK_BATCH);
        partition_best(entries, &mut pool, n);
        sort_scored_hits(entries, &mut pool[..n]);
        let examined = fill_hits_from_batch(entries, &pool[..n], &mut hits);
        pool.drain(..examined);
    }
    let capped = hits.len() == SEARCH_MAX_HITS && has_openable_left(entries, &pool);
    (hits, capped)
}

#[cfg(test)]
fn hit_paths(entries: &[NeighbourEntry], scored: Vec<ScoredHit>) -> Vec<PathBuf> {
    scored
        .into_iter()
        .map(|h| entries[h.2].path.clone())
        .collect()
}

/// Every name match (not yet openability-checked); known-unopenable skipped.
fn collect_name_hits(
    entries: &[NeighbourEntry],
    q: &str,
    progress: &HashSet<String>,
) -> Vec<ScoredHit> {
    let q_tri = if q.chars().count() >= 3 {
        query_trigrams(q)
    } else {
        HashSet::new()
    };
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.known_unopenable())
        .filter_map(|(i, e)| score_name_hit(e, i, q, &q_tri, progress))
        .collect()
}

#[cfg(test)]
fn score_name_hits(
    entries: &[NeighbourEntry],
    q: &str,
    progress: &HashSet<String>,
) -> Vec<ScoredHit> {
    collect_name_hits(entries, q, progress)
}

/// Move the best `n` hits (by [hit_ord]) to the front of `pool`.
fn partition_best(entries: &[NeighbourEntry], pool: &mut [ScoredHit], n: usize) {
    if n == 0 || pool.len() <= n {
        return;
    }
    pool.select_nth_unstable_by(n - 1, |a, b| hit_ord(entries, a, b));
}

/// Preflight batch members in rank order until the strip is full.
/// Returns how many batch slots were examined (unexamined stay in the pool).
fn fill_hits_from_batch(
    entries: &[NeighbourEntry],
    batch: &[ScoredHit],
    hits: &mut Vec<PathBuf>,
) -> usize {
    let mut examined = 0;
    for h in batch {
        if hits.len() >= SEARCH_MAX_HITS {
            break;
        }
        examined += 1;
        let e = &entries[h.2];
        if e.known_unopenable() {
            continue;
        }
        if e.is_openable() {
            hits.push(e.path.clone());
        }
    }
    examined
}

/// Whether any remaining name match is playable (stops at the first yes).
fn has_openable_left(entries: &[NeighbourEntry], pool: &[ScoredHit]) -> bool {
    pool.iter().any(|h| {
        let e = &entries[h.2];
        !e.known_unopenable() && e.is_openable()
    })
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
    Some((score, started, idx))
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
fn hit_ord(entries: &[NeighbourEntry], a: &ScoredHit, b: &ScoredHit) -> std::cmp::Ordering {
    b.0.partial_cmp(&a.0)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| b.1.cmp(&a.1))
        .then_with(|| {
            lexical_sort::natural_lexical_cmp(&entries[a.2].name_lower, &entries[b.2].name_lower)
        })
}

fn sort_scored_hits(entries: &[NeighbourEntry], scored: &mut [ScoredHit]) {
    scored.sort_by(|a, b| hit_ord(entries, a, b));
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
        let entries = [
            NeighbourEntry::known(PathBuf::from("/store/ep05.mkv"), true),
            NeighbourEntry::known(PathBuf::from("/store/ep07.mkv"), true),
            NeighbourEntry::known(PathBuf::from("/store/ep06.mkv"), true),
        ];
        let mut scored = [
            (0.5, false, 0usize),
            (0.5, true, 1usize),
            (0.5, false, 2usize),
        ];
        sort_scored_hits(&entries, &mut scored);
        assert_eq!(scored[0].2, 1);
        assert!(scored[0].1);
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
        let (hits, capped) = capped_name_hits(&entries, "pick", &HashSet::new());
        assert!(capped);
        assert_eq!(hits.len(), SEARCH_MAX_HITS);
    }

    #[test]
    fn wide_one_letter_query_does_not_keep_every_match() {
        let entries: Vec<_> = (0..500)
            .map(|i| NeighbourEntry::known(PathBuf::from(format!("/store/a{i:03}.mkv")), true))
            .collect();
        let (hits, capped) = capped_name_hits(&entries, "a", &HashSet::new());
        assert!(capped);
        assert_eq!(hits.len(), SEARCH_MAX_HITS);
    }

    #[test]
    fn strip_fills_past_unopenable_ranked_prefix() {
        let dir = rank_batch_scratch();
        let mut entries = hollow_named(&dir, 0, RANK_BATCH + 5);
        let open_from = RANK_BATCH + 5;
        entries.extend(open_named(&dir, open_from, SEARCH_MAX_HITS));
        let (hits, capped) = capped_name_hits(&entries, "a", &HashSet::new());
        assert_eq!(hits.len(), SEARCH_MAX_HITS);
        assert!(!capped);
        assert!(hits.iter().all(|p| hit_num(p) >= open_from));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn capped_false_when_playable_under_strip_limit() {
        let dir = rank_batch_scratch();
        let mut entries = hollow_named(&dir, 0, SEARCH_MAX_HITS + 10);
        let open_from = SEARCH_MAX_HITS + 10;
        entries.extend(open_named(&dir, open_from, 5));
        let (hits, capped) = capped_name_hits(&entries, "a", &HashSet::new());
        assert_eq!(hits.len(), 5);
        assert!(!capped);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn capped_false_when_remainder_unopenable() {
        let dir = rank_batch_scratch();
        let mut entries = open_named(&dir, 0, SEARCH_MAX_HITS);
        entries.extend(hollow_named(&dir, SEARCH_MAX_HITS, 10));
        let (hits, capped) = capped_name_hits(&entries, "a", &HashSet::new());
        assert_eq!(hits.len(), SEARCH_MAX_HITS);
        assert!(!capped);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn rank_batch_scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rhino-rank-batch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn hollow_named(dir: &Path, from: usize, n: usize) -> Vec<NeighbourEntry> {
        (from..from + n)
            .map(|i| {
                let p = dir.join(format!("a{i:03}.mkv"));
                std::fs::write(&p, vec![0u8; 128 * 1024]).unwrap();
                NeighbourEntry::pending(p)
            })
            .collect()
    }

    fn open_named(dir: &Path, from: usize, n: usize) -> Vec<NeighbourEntry> {
        (from..from + n)
            .map(|i| {
                let p = dir.join(format!("a{i:03}.mkv"));
                std::fs::write(&p, b"RIFF....AVI \x01\x02\x03\x04").unwrap();
                NeighbourEntry::pending(p)
            })
            .collect()
    }

    fn hit_num(p: &Path) -> usize {
        p.file_name()
            .unwrap()
            .to_string_lossy()
            .trim_start_matches('a')
            .trim_end_matches(".mkv")
            .parse()
            .unwrap()
    }
}
