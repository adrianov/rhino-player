// Fill a trashed or removed lucky slot from the reserved next handful or one unused title.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::{fisher_yates, lucky_seed, lucky_titles, same_shown, NeighbourEntry};

/// Replace a trashed or removed lucky card in-place; prefers a reserved next path, then one unused title.
pub(crate) fn fill_lucky_gap(
    lucky: &mut Vec<PathBuf>,
    next: &mut Option<Vec<PathBuf>>,
    gone: &std::path::Path,
    entries: &[NeighbourEntry],
    seen: &mut HashSet<String>,
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) {
    let Some(at) = lucky.iter().position(|p| same_shown(p, gone)) else {
        lucky.retain(|p| !same_shown(p, gone));
        return;
    };
    lucky.remove(at);
    let from_next = pop_next(next, lucky, entries);
    if let Some(p) = from_next
        .clone()
        .or_else(|| take_one_title(entries, seen, lucky, tpos, durs))
    {
        lucky.insert(at.min(lucky.len()), p);
        if from_next.is_some() {
            top_up_next(next, lucky, entries, seen, tpos, durs);
        }
    }
}

fn pop_next(
    next: &mut Option<Vec<PathBuf>>,
    skip: &[PathBuf],
    entries: &[NeighbourEntry],
) -> Option<PathBuf> {
    let v = next.as_mut()?;
    let i = v.iter().position(|p| slot_candidate(p, skip, entries))?;
    let p = v.remove(i);
    if v.is_empty() {
        *next = None;
    }
    Some(p)
}

fn slot_candidate(path: &std::path::Path, skip: &[PathBuf], entries: &[NeighbourEntry]) -> bool {
    !skip.iter().any(|s| same_shown(s, path))
        && entries
            .iter()
            .find(|e| same_shown(&e.path, path))
            .is_some_and(NeighbourEntry::is_openable)
}

fn top_up_next(
    next: &mut Option<Vec<PathBuf>>,
    lucky: &[PathBuf],
    entries: &[NeighbourEntry],
    seen: &mut HashSet<String>,
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) {
    let mut skip = lucky.to_vec();
    if let Some(n) = next.as_ref() {
        skip.extend(n.iter().cloned());
    }
    let Some(p) = take_one_title(entries, seen, &skip, tpos, durs) else {
        return;
    };
    next.get_or_insert_with(Vec::new).push(p);
}

fn take_one_title(
    entries: &[NeighbourEntry],
    seen: &mut HashSet<String>,
    skip: &[PathBuf],
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) -> Option<PathBuf> {
    let mut pool = unused_or_all(
        lucky_titles(entries, tpos, durs)
            .into_iter()
            .filter(|(_, p)| !skip.contains(p))
            .collect(),
        seen,
    );
    if pool.is_empty() {
        return None;
    }
    fisher_yates(&mut pool, lucky_seed());
    let (id, path) = pool.swap_remove(0);
    seen.insert(id);
    Some(path)
}

fn unused_or_all(titles: Vec<(String, PathBuf)>, seen: &HashSet<String>) -> Vec<(String, PathBuf)> {
    let unused: Vec<_> = titles
        .iter()
        .filter(|(id, _)| !seen.contains(id))
        .cloned()
        .collect();
    if unused.is_empty() {
        titles
    } else {
        unused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, openable: bool) -> NeighbourEntry {
        NeighbourEntry::known(PathBuf::from(path), openable)
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn empty_maps() -> (HashMap<String, f64>, HashMap<String, f64>) {
        (HashMap::new(), HashMap::new())
    }

    fn run_fill(
        lucky: Vec<PathBuf>,
        next: Option<Vec<PathBuf>>,
        gone: &str,
        entries: &[NeighbourEntry],
        seen: &mut HashSet<String>,
        tpos: &HashMap<String, f64>,
        durs: &HashMap<String, f64>,
    ) -> Vec<PathBuf> {
        let mut lucky = lucky;
        let mut next = next;
        fill_lucky_gap(
            &mut lucky,
            &mut next,
            &p(gone),
            entries,
            seen,
            tpos,
            durs,
        );
        lucky
    }

    #[test]
    fn fill_gap_takes_reserved_next_in_same_slot() {
        let entries = [
            entry("/a.mkv", false),
            entry("/b.mkv", true),
            entry("/c.mkv", true),
        ];
        let (tpos, durs) = empty_maps();
        assert_eq!(
            run_fill(
                vec![p("/a.mkv"), p("/b.mkv")],
                Some(vec![p("/c.mkv")]),
                "/a.mkv",
                &entries,
                &mut HashSet::new(),
                &tpos,
                &durs,
            ),
            vec![p("/c.mkv"), p("/b.mkv")]
        );
    }

    #[test]
    fn fill_gap_rolls_unused_when_next_empty() {
        let entries = [
            entry("/a.mkv", true),
            entry("/b.mkv", true),
            entry("/c.mkv", true),
        ];
        let (tpos, durs) = empty_maps();
        let mut seen = HashSet::from(["f:/a.mkv".into(), "f:/b.mkv".into()]);
        assert_eq!(
            run_fill(
                vec![p("/a.mkv"), p("/b.mkv")],
                None,
                "/a.mkv",
                &entries,
                &mut seen,
                &tpos,
                &durs,
            ),
            vec![p("/c.mkv"), p("/b.mkv")]
        );
    }

    #[test]
    fn fill_gap_stays_short_when_nothing_left() {
        let entries = [entry("/a.mkv", false), entry("/b.mkv", true)];
        let (tpos, durs) = empty_maps();
        assert_eq!(
            run_fill(
                vec![p("/a.mkv"), p("/b.mkv")],
                None,
                "/a.mkv",
                &entries,
                &mut HashSet::new(),
                &tpos,
                &durs,
            ),
            vec![p("/b.mkv")]
        );
    }
}
