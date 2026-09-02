// I'm Feeling Lucky: shuffle openable neighbour-index paths (feature 33).

use std::path::PathBuf;

use super::NeighbourEntry;

/// Openable index paths, shuffled, then capped.
pub(super) fn lucky_picks(entries: &[NeighbourEntry], max: usize) -> Vec<PathBuf> {
    lucky_picks_with_seed(entries, max, lucky_seed())
}

fn lucky_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64 | 1)
        .unwrap_or(1)
}

fn lucky_picks_with_seed(entries: &[NeighbourEntry], max: usize, seed: u64) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = entries
        .iter()
        .filter(|e| e.openable)
        .map(|e| e.path.clone())
        .collect();
    fisher_yates(&mut paths, seed);
    paths.truncate(max);
    paths
}

fn fisher_yates(paths: &mut [PathBuf], seed: u64) {
    let mut rng = seed | 1;
    for i in (1..paths.len()).rev() {
        rng = rng.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
        let j = (rng as usize) % (i + 1);
        paths.swap(i, j);
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

pub(super) fn lucky_hint(n: usize) -> String {
    match n {
        0 => "Nothing to pick".into(),
        1 => "1 lucky pick".into(),
        n => format!("{n} lucky picks"),
    }
}

pub(super) fn search_hint(n: usize, capped: bool) -> String {
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

    #[test]
    fn lucky_picks_skips_unopenable_and_caps() {
        let entries: Vec<_> = (0..10)
            .map(|i| entry(&format!("/store/v{i}.mkv"), i % 2 == 0))
            .collect();
        let picks = lucky_picks_with_seed(&entries, 3, 42);
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
        assert!(lucky_picks_with_seed(&[entry("/store/a.mkv", false)], 5, 1).is_empty());
    }

    #[test]
    fn lucky_picks_returns_all_when_fewer_than_cap() {
        let entries = [entry("/a.mkv", true), entry("/b.mkv", true)];
        assert_eq!(lucky_picks_with_seed(&entries, 5, 7).len(), 2);
    }

    #[test]
    fn lucky_picks_same_seed_is_stable() {
        let entries: Vec<_> = (0..8)
            .map(|i| entry(&format!("/s/{i}.mkv"), true))
            .collect();
        assert_eq!(
            lucky_picks_with_seed(&entries, 5, 99),
            lucky_picks_with_seed(&entries, 5, 99)
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
}
