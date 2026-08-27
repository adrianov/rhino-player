// Unit tests for the neighbour-search pure logic (scan, filter, exclusion). Included from
// `sibling_search.rs`'s `mod tests`.

fn scratch(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let d = std::env::temp_dir().join(format!(
        "rhino-sibsearch-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn touch(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), b"x").unwrap();
}

#[test]
fn substring_match_ignores_case() {
    assert!(file_name_lower(Path::new("/x/Show.S01E04.FiNAL.mkv")).contains("s01e04"));
    assert!(file_name_lower(Path::new("/x/EPISODE 7.MP4")).contains("episode"));
    assert!(!file_name_lower(Path::new("/x/other.avi")).contains("episode"));
}

#[test]
fn scan_lists_only_video_files_naturally() {
    let dir = scratch("scan");
    touch(&dir, "ep10.mkv");
    touch(&dir, "ep2.MKV");
    touch(&dir, "note.txt");
    std::fs::create_dir(dir.join("season 3")).unwrap();
    let videos = crate::video_ext::list_videos_in_dir(&dir).unwrap();
    let names: Vec<String> = videos.iter().map(|p| file_name_lower(p)).collect();
    assert_eq!(names, vec!["ep2.mkv", "ep10.mkv"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn parents_dedupe_across_entries() {
    let dirs = watch_later_parent_dirs(&[
        PathBuf::from("/lib/a/one.mkv"),
        PathBuf::from("/lib/a/two.mkv"),
        PathBuf::from("/lib/b/nested.mkv"),
    ]);
    assert_eq!(dirs.len(), 2);
}

#[test]
fn hits_exclude_current_list_members() {
    let files = vec![
        PathBuf::from("/store/pick1.mkv"),
        PathBuf::from("/store/listed.mkv"),
    ];
    let mut exclude = HashSet::new();
    exclude.insert(entity_key(Path::new("/store/listed.mkv")));
    let hits = collect_hits(&files, "pick", &exclude);
    assert_eq!(hits, vec![PathBuf::from("/store/pick1.mkv")]);
}

#[test]
fn collect_hits_does_not_cap_cap_is_callers_concern() {
    let files: Vec<PathBuf> = (0..SEARCH_MAX_HITS + 5)
        .map(|i| PathBuf::from(format!("/store/pick{i}.mkv")))
        .collect();
    let exclude = HashSet::new();
    let hits = collect_hits(&files, "pick", &exclude);
    assert_eq!(hits.len(), SEARCH_MAX_HITS + 5);
}
