// Neighbour (sibling) search for the continue screen — feature hub.
// See docs/features/33-continue-sibling-search.md. Split across:
//   sibling_search.rs          — this file: scan core, hit filter, strip-paint plan, tests
//   sibling_search_state.rs    — [SiblingSearchState]: query, index refresh, debounce, strip API
//   sibling_search_widgets.rs  — the search-row widgets (pill entry + inline hint)
// NOTE: include!'d into `recent_view`; shares its imports (glib, Rc, RefCell, Path, Duration).

include!("sibling_search_widgets.rs");
include!("sibling_search_state.rs");

use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Weak;
use std::time::Instant;

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

/// Distinct immediate parents of watch-later paths, in stable order.
fn watch_later_parent_dirs(entries: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for p in entries {
        if let Some(d) = p.parent() {
            if !dirs.iter().any(|x| x == d) {
                dirs.push(d.to_path_buf());
            }
        }
    }
    dirs
}

/// Video files under every distinct watch-later parent, natural order.
fn scan_watch_later_dirs() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in watch_later_parent_dirs(&crate::history::load()) {
        if let Some(videos) = crate::video_ext::list_videos_in_dir(&dir) {
            files.extend(videos);
        }
    }
    sort_neighbours(&mut files);
    files
}

/// Name-substring matches minus current watch-later entries, natural order.
fn collect_hits(files: &[PathBuf], q: &str, exclude: &HashSet<String>) -> Vec<PathBuf> {
    let mut hits: Vec<PathBuf> = files
        .iter()
        .filter(|p| file_name_lower(p).contains(q))
        .filter(|p| !exclude.contains(&entity_key(p)))
        .cloned()
        .collect();
    sort_neighbours(&mut hits);
    hits
}

fn file_name_lower(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Canonical-ish identity used to keep real watch-later members out of the results.
fn entity_key(p: &Path) -> String {
    crate::playback_entity::db_path_for(p)
        .to_string_lossy()
        .into_owned()
}

fn history_entity_keys() -> HashSet<String> {
    crate::history::load().iter().map(|p| entity_key(p)).collect()
}

fn sort_neighbours(v: &mut [PathBuf]) {
    v.sort_by(|a, b| lexical_sort::natural_lexical_cmp(&file_name_lower(a), &file_name_lower(b)));
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("sibling_search_tests.rs");
}
