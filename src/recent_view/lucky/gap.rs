// Fill a trashed or removed lucky slot from the reserved next handful or one unused title.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::{fisher_yates, keep_openable, lucky_seed, lucky_titles, same_shown, NeighbourEntry};

/// Replace a trashed or removed lucky card in-place; prefers a reserved next path, then one unused title.
pub(crate) fn fill_lucky_gap(
    lucky: &mut Vec<PathBuf>,
    next: &mut Option<Vec<PathBuf>>,
    gone: &std::path::Path,
    entries: &[NeighbourEntry],
    seen: &mut HashSet<String>,
) {
    let tpos = crate::db::load_time_pos_map();
    let durs = crate::db::load_duration_map();
    fill_lucky_gap_maps(lucky, next, gone, entries, seen, &tpos, &durs);
}

fn fill_lucky_gap_maps(
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
    let repl = from_next
        .clone()
        .or_else(|| take_one_title(entries, seen, lucky, tpos, durs));
    if let Some(p) = repl {
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
    let pick = keep_openable(v, entries)
        .into_iter()
        .find(|p| !skip.contains(p))?;
    v.retain(|p| p != &pick);
    if v.is_empty() {
        *next = None;
    }
    Some(pick)
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
    let titles: Vec<_> = lucky_titles(entries, tpos, durs)
        .into_iter()
        .filter(|(_, p)| !skip.contains(p))
        .collect();
    let mut pool = unused_or_all(titles, seen);
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
        NeighbourEntry {
            path: PathBuf::from(path),
            openable,
        }
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn empty_maps() -> (HashMap<String, f64>, HashMap<String, f64>) {
        (HashMap::new(), HashMap::new())
    }

    #[test]
    fn fill_gap_takes_reserved_next_in_same_slot() {
        let entries = [
            entry("/a.mkv", false),
            entry("/b.mkv", true),
            entry("/c.mkv", true),
        ];
        let (tpos, durs) = empty_maps();
        let mut lucky = vec![p("/a.mkv"), p("/b.mkv")];
        let mut next = Some(vec![p("/c.mkv")]);
        fill_lucky_gap_maps(
            &mut lucky,
            &mut next,
            &p("/a.mkv"),
            &entries,
            &mut HashSet::new(),
            &tpos,
            &durs,
        );
        assert_eq!(lucky[0], p("/c.mkv"));
        assert_eq!(lucky[1], p("/b.mkv"));
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
        let mut lucky = vec![p("/a.mkv"), p("/b.mkv")];
        let mut next = None;
        fill_lucky_gap_maps(
            &mut lucky,
            &mut next,
            &p("/a.mkv"),
            &entries,
            &mut seen,
            &tpos,
            &durs,
        );
        assert_eq!(lucky, vec![p("/c.mkv"), p("/b.mkv")]);
    }

    #[test]
    fn fill_gap_stays_short_when_nothing_left() {
        let entries = [entry("/a.mkv", false), entry("/b.mkv", true)];
        let (tpos, durs) = empty_maps();
        let mut lucky = vec![p("/a.mkv"), p("/b.mkv")];
        fill_lucky_gap_maps(
            &mut lucky,
            &mut None,
            &p("/a.mkv"),
            &entries,
            &mut HashSet::new(),
            &tpos,
            &durs,
        );
        assert_eq!(lucky, vec![p("/b.mkv")]);
    }
}
