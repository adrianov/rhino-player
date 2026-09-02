// I'm Feeling Lucky owner (feature 33): sample, series collapse, session reserve, trash refill.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub(super) use super::NeighbourEntry;

mod titles;
use titles::{lucky_titles, retarget_paths};
mod gap;
mod session;
pub(crate) use session::LuckySession;

/// Openable titles, unseen this session first; series collapse to continue-or-first.
pub(super) fn lucky_picks(
    entries: &[NeighbourEntry],
    max: usize,
    seen: &mut HashSet<String>,
) -> Vec<PathBuf> {
    let tpos = crate::db::load_time_pos_map();
    let durs = crate::db::load_duration_map();
    lucky_picks_with_seed(entries, max, lucky_seed(), seen, &tpos, &durs)
}

/// Use a reserved handful when it still has playable paths; otherwise roll a new sample.
pub(super) fn take_ready_or_roll(
    ready: &mut Option<Vec<PathBuf>>,
    entries: &[NeighbourEntry],
    max: usize,
    seen: &mut HashSet<String>,
) -> Vec<PathBuf> {
    if let Some(paths) = ready.take() {
        let open = keep_openable(&paths, entries);
        if !open.is_empty() {
            return open;
        }
    }
    lucky_picks(entries, max, seen)
}

/// Reserve the next handful (marks those titles seen).
pub(super) fn reserve_lucky(
    entries: &[NeighbourEntry],
    max: usize,
    seen: &mut HashSet<String>,
) -> Option<Vec<PathBuf>> {
    let next = lucky_picks(entries, max, seen);
    (!next.is_empty()).then_some(next)
}

fn lucky_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64 | 1)
        .unwrap_or(1)
}

fn lucky_picks_with_seed(
    entries: &[NeighbourEntry],
    max: usize,
    seed: u64,
    seen: &mut HashSet<String>,
    tpos: &HashMap<String, f64>,
    durs: &HashMap<String, f64>,
) -> Vec<PathBuf> {
    let mut titles = lucky_titles(entries, tpos, durs);
    titles.sort_by(|a, b| a.0.cmp(&b.0));
    let mut pool = drain_unseen(titles, seen);
    fisher_yates(&mut pool, seed);
    pool.truncate(max);
    remember_seen(&pool, seen);
    pool.into_iter().map(|(_, p)| p).collect()
}

fn drain_unseen(
    titles: Vec<(String, PathBuf)>,
    seen: &mut HashSet<String>,
) -> Vec<(String, PathBuf)> {
    let unused: Vec<_> = titles
        .iter()
        .filter(|(id, _)| !seen.contains(id))
        .cloned()
        .collect();
    if unused.is_empty() {
        seen.clear();
        titles
    } else {
        unused
    }
}

fn remember_seen(pool: &[(String, PathBuf)], seen: &mut HashSet<String>) {
    for (id, _) in pool {
        seen.insert(id.clone());
    }
}

fn fisher_yates<T>(items: &mut [T], seed: u64) {
    let mut rng = seed | 1;
    for i in (1..items.len()).rev() {
        rng = rng.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
        let j = (rng as usize) % (i + 1);
        items.swap(i, j);
    }
}

/// Point shown/reserved lucky paths at each title's current continue-or-first episode.
pub(super) fn retarget_lucky(
    shown: &mut Option<Vec<PathBuf>>,
    next: &mut Option<Vec<PathBuf>>,
    index: &[NeighbourEntry],
) {
    let tpos = crate::db::load_time_pos_map();
    let durs = crate::db::load_duration_map();
    if let Some(paths) = shown.as_mut() {
        retarget_paths(paths, index, &tpos, &durs);
    }
    if let Some(paths) = next.as_mut() {
        retarget_paths(paths, index, &tpos, &durs);
    }
}

/// Drop snapshot paths that the live index now marks unopenable (trash).
pub(super) fn keep_openable(paths: &[PathBuf], index: &[NeighbourEntry]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|p| index.iter().any(|e| e.path == **p && e.openable))
        .cloned()
        .collect()
}

pub(crate) fn lucky_hint(n: usize) -> String {
    match n {
        0 => "Nothing to pick".into(),
        1 => "1 lucky pick".into(),
        n => format!("{n} lucky picks"),
    }
}

pub(crate) fn search_hint(n: usize, capped: bool) -> String {
    match (n, capped) {
        (_, true) => format!("{n}+ matches"),
        (0, _) => "No matches".into(),
        (1, _) => "1 match".into(),
        (n, _) => format!("{n} matches"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::file_name_lower;
    use super::*;

    fn entry(path: &str, openable: bool) -> NeighbourEntry {
        NeighbourEntry {
            path: PathBuf::from(path),
            openable,
        }
    }

    fn empty_maps() -> (HashMap<String, f64>, HashMap<String, f64>) {
        (HashMap::new(), HashMap::new())
    }

    fn draw(
        entries: &[NeighbourEntry],
        max: usize,
        seed: u64,
        seen: &mut HashSet<String>,
        tpos: &HashMap<String, f64>,
        durs: &HashMap<String, f64>,
    ) -> Vec<PathBuf> {
        lucky_picks_with_seed(entries, max, seed, seen, tpos, durs)
    }

    #[test]
    fn lucky_picks_skips_unopenable_and_caps() {
        let entries: Vec<_> = (0..10)
            .map(|i| entry(&format!("/store/v{i}.mkv"), i % 2 == 0))
            .collect();
        let (tpos, durs) = empty_maps();
        let picks = draw(&entries, 3, 42, &mut HashSet::new(), &tpos, &durs);
        assert_eq!(picks.len(), 3);
        assert!(picks.iter().all(|p| even_store_name(p)));
    }

    fn even_store_name(p: &std::path::Path) -> bool {
        file_name_lower(p)
            .trim_start_matches('v')
            .trim_end_matches(".mkv")
            .parse::<u32>()
            .is_ok_and(|n| n % 2 == 0)
    }

    #[test]
    fn lucky_picks_empty_when_none_openable() {
        let (tpos, durs) = empty_maps();
        assert!(draw(
            &[entry("/store/a.mkv", false)],
            5,
            1,
            &mut HashSet::new(),
            &tpos,
            &durs
        )
        .is_empty());
    }

    #[test]
    fn lucky_picks_returns_all_when_fewer_than_cap() {
        let entries = [entry("/a.mkv", true), entry("/b.mkv", true)];
        let (tpos, durs) = empty_maps();
        assert_eq!(
            draw(&entries, 5, 7, &mut HashSet::new(), &tpos, &durs).len(),
            2
        );
    }

    #[test]
    fn lucky_picks_same_seed_is_stable() {
        let entries: Vec<_> = (0..8)
            .map(|i| entry(&format!("/s/{i}.mkv"), true))
            .collect();
        let (tpos, durs) = empty_maps();
        assert_eq!(
            draw(&entries, 5, 99, &mut HashSet::new(), &tpos, &durs),
            draw(&entries, 5, 99, &mut HashSet::new(), &tpos, &durs)
        );
    }

    #[test]
    fn keep_openable_drops_trashed_snapshot_paths() {
        let paths = vec![
            PathBuf::from("/store/ok.mkv"),
            PathBuf::from("/store/gone.mkv"),
        ];
        let index = [
            entry("/store/ok.mkv", true),
            entry("/store/gone.mkv", false),
        ];
        assert_eq!(
            keep_openable(&paths, &index),
            vec![PathBuf::from("/store/ok.mkv")]
        );
    }

    #[test]
    fn series_unstarted_uses_first_episode_only() {
        let entries = [
            entry("/t/Show/Show.S01E02.mkv", true),
            entry("/t/Show/Show.S01E01.mkv", true),
            entry("/t/Movie.mkv", true),
        ];
        let (tpos, durs) = empty_maps();
        let picks = draw(&entries, 5, 1, &mut HashSet::new(), &tpos, &durs);
        assert_eq!(picks.len(), 2);
        assert!(picks.iter().any(|p| p.ends_with("Show.S01E01.mkv")));
        assert!(!picks.iter().any(|p| p.ends_with("Show.S01E02.mkv")));
    }

    #[test]
    fn retarget_paths_moves_series_to_watching_episode() {
        let e1 = "/t/Show/Show.S01E01.mkv";
        let e2 = "/t/Show/Show.S01E02.mkv";
        let entries = [entry(e1, true), entry(e2, true)];
        let mut tpos = HashMap::new();
        let mut durs = HashMap::new();
        tpos.insert(e2.to_string(), 180.0);
        durs.insert(e2.to_string(), 2400.0);
        let mut paths = vec![PathBuf::from(e1)];
        titles::retarget_paths(&mut paths, &entries, &tpos, &durs);
        assert_eq!(paths, vec![PathBuf::from(e2)]);
    }

    #[test]
    fn retarget_paths_keeps_standalone() {
        let path = "/store/Movie.mkv";
        let entries = [entry(path, true)];
        let (tpos, durs) = empty_maps();
        let mut paths = vec![PathBuf::from(path)];
        titles::retarget_paths(&mut paths, &entries, &tpos, &durs);
        assert_eq!(paths, vec![PathBuf::from(path)]);
    }

    #[test]
    fn series_in_progress_uses_watching_episode() {
        let e2 = "/t/Show/Show.S01E02.mkv";
        let entries = [
            entry("/t/Show/Show.S01E01.mkv", true),
            entry(e2, true),
            entry("/t/Show/Show.S01E03.mkv", true),
        ];
        let mut tpos = HashMap::new();
        let mut durs = HashMap::new();
        tpos.insert(e2.to_string(), 180.0);
        durs.insert(e2.to_string(), 2400.0);
        let picks = draw(&entries, 5, 1, &mut HashSet::new(), &tpos, &durs);
        assert_eq!(picks, vec![PathBuf::from(e2)]);
    }

    #[test]
    fn season_folders_collapse_to_one_first_episode() {
        let entries = [
            entry("/lib/Show Season 2/e01.mkv", true),
            entry("/lib/Show Season 1/e02.mkv", true),
            entry("/lib/Show Season 1/e01.mkv", true),
        ];
        let (tpos, durs) = empty_maps();
        let picks = draw(&entries, 5, 3, &mut HashSet::new(), &tpos, &durs);
        assert_eq!(picks, vec![PathBuf::from("/lib/Show Season 1/e01.mkv")]);
    }

    #[test]
    fn later_draw_skips_seen_until_pool_empty() {
        let entries: Vec<_> = (0..8)
            .map(|i| entry(&format!("/lib/m{i}.mkv"), true))
            .collect();
        let (tpos, durs) = empty_maps();
        let mut seen = HashSet::new();
        let first = draw(&entries, 5, 11, &mut seen, &tpos, &durs);
        let second = draw(&entries, 5, 22, &mut seen, &tpos, &durs);
        assert_eq!(first.len(), 5);
        assert_eq!(second.len(), 3);
        assert!(first.iter().all(|p| !second.contains(p)));
        let third = draw(&entries, 5, 33, &mut seen, &tpos, &durs);
        assert_eq!(third.len(), 5);
    }

    #[test]
    fn take_ready_uses_reserved_without_rolling() {
        let entries: Vec<_> = (0..8)
            .map(|i| entry(&format!("/lib/m{i}.mkv"), true))
            .collect();
        let (tpos, durs) = empty_maps();
        let mut seen = HashSet::new();
        let _ = draw(&entries, 5, 11, &mut seen, &tpos, &durs);
        let reserved = draw(&entries, 5, 22, &mut seen, &tpos, &durs);
        let n = seen.len();
        let mut ready = Some(reserved.clone());
        assert_eq!(
            take_ready_or_roll(&mut ready, &entries, 5, &mut seen),
            reserved
        );
        assert!(ready.is_none());
        assert_eq!(seen.len(), n);
    }

    #[test]
    fn take_ready_rolls_when_reserved_unopenable() {
        let entries = [
            entry("/lib/a.mkv", true),
            entry("/lib/b.mkv", true),
            entry("/lib/c.mkv", false),
        ];
        let mut seen = HashSet::new();
        let mut ready = Some(vec![PathBuf::from("/lib/c.mkv")]);
        let got = take_ready_or_roll(&mut ready, &entries, 5, &mut seen);
        assert_eq!(got.len(), 2);
        assert!(got.iter().all(|p| p.as_os_str() != "/lib/c.mkv"));
    }
}
