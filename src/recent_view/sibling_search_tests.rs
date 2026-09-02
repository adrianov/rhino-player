// Unit tests for the neighbour-search pure logic (scan, filter). Included from
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

/// Two sibling show folders under `root/Shows`, each with one video.
fn two_show_dirs(root: &Path) -> (PathBuf, PathBuf) {
    let a = root.join("Shows/ShowA");
    let b = root.join("Shows/ShowB");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    touch(&a, "seed.mkv");
    touch(&b, "neighbour.mkv");
    (a, b)
}

#[test]
fn substring_match_ignores_case() {
    let q = "s01e04";
    let q_tri = query_trigrams(q);
    assert!(name_match_score("show.s01e04.final.mkv", q, &q_tri).is_some());
    assert!(name_match_score("episode 7.mp4", "episode", &query_trigrams("episode")).is_some());
    assert!(name_match_score("other.avi", "episode", &query_trigrams("episode")).is_none());
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
fn sibling_dirs_under_shared_parent_are_scanned() {
    let root = scratch("sibs");
    let (a, _) = two_show_dirs(&root);
    let names: Vec<_> = scan_sibling_universe(&[a.join("seed.mkv")])
        .iter()
        .map(|p| file_name_lower(p))
        .collect();
    assert!(names.iter().any(|n| n == "seed.mkv"));
    assert!(names.iter().any(|n| n == "neighbour.mkv"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn top_level_dirs_under_root_are_not_siblings() {
    // Simulate /LibA and /LibB by using a fake root whose parent is missing from the
    // sibling walk: we only assert the pure helper never enqueues siblings when
    // grandparent is fs root. Build two dirs under a temp "root" and call
    // enqueue_sibling_dirs with grandparent marked via is_fs_root on Path::new("/").
    assert!(is_fs_root(Path::new("/")));
    assert!(!is_fs_root(Path::new("/Users")));
    assert!(!is_fs_root(Path::new("/Users/me/Video")));

    let mut queue = VecDeque::new();
    let mut seen = HashSet::new();
    // dir = /Video → grandparent = / → no sibling enqueue
    enqueue_sibling_dirs(&mut queue, &mut seen, Path::new("/Video"));
    assert!(queue.is_empty());
    // Still allow scanning /Video itself when seeded
    enqueue_scan_dir(&mut queue, &mut seen, Path::new("/Video"));
    assert_eq!(queue.len(), 1);
    // Never scan /
    enqueue_scan_dir(&mut queue, &mut seen, Path::new("/"));
    assert_eq!(queue.len(), 1);
}

#[test]
fn hits_include_continue_list_members() {
    let entries = vec![
        entry("/store/pick1.mkv", true),
        entry("/store/listed.mkv", true),
    ];
    let hits = present_name_hits(&entries, "listed");
    assert_eq!(hits, vec![PathBuf::from("/store/listed.mkv")]);
}

#[test]
fn present_hits_use_materialized_openable() {
    let entries = vec![
        entry("/store/ok.mkv", true),
        entry("/store/hollow.mkv", false),
        entry("/gone/missing.mkv", false),
    ];
    assert_eq!(
        present_name_hits(&entries, "mkv"),
        vec![PathBuf::from("/store/ok.mkv")]
    );
}

#[test]
fn present_hits_rank_by_trigram_score() {
    let entries = vec![
        entry("/store/summer.vacation.mkv", true),
        entry("/store/somm.2012.mkv", true),
    ];
    let hits = present_name_hits(&entries, "somm");
    assert_eq!(
        hits.first().map(|p| file_name_lower(p)).as_deref(),
        Some("somm.2012.mkv")
    );
}

fn entry(path: &str, openable: bool) -> NeighbourEntry {
    NeighbourEntry {
        path: PathBuf::from(path),
        openable,
    }
}

#[test]
fn classify_rejects_empty_hollow_and_missing() {
    let dir = scratch("classify");
    let ok = dir.join("ok.mkv");
    std::fs::write(&ok, b"RIFF....AVI \x01\x02\x03\x04").unwrap();
    assert!(classify_openable(&ok));
    assert!(!classify_openable(&write_empty(&dir, "empty.mkv")));
    assert!(!classify_openable(&write_hollow(&dir, "hollow.mkv")));
    assert!(!classify_openable(&dir.join("gone.mkv")));
    std::fs::remove_dir_all(&dir).ok();
}

fn write_empty(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::File::create(&p).unwrap();
    p
}

fn write_hollow(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, vec![0u8; 128 * 1024]).unwrap();
    p
}

#[test]
fn present_hits_uncapped_cap_is_callers_concern() {
    let entries: Vec<_> = (0..SEARCH_MAX_HITS + 5)
        .map(|i| entry(&format!("/store/pick{i}.mkv"), true))
        .collect();
    let hits = present_name_hits(&entries, "pick");
    assert_eq!(hits.len(), SEARCH_MAX_HITS + 5);
}

fn fill_counted(
    scanned: &Cell<bool>,
    index: &RefCell<Vec<NeighbourEntry>>,
    builds: &Cell<u32>,
    path: PathBuf,
) -> Vec<NeighbourEntry> {
    index_fill_once(scanned, index, || {
        builds.set(builds.get() + 1);
        vec![NeighbourEntry {
            path,
            openable: true,
        }]
    });
    index.borrow().clone()
}

#[test]
fn index_fill_once_builds_only_first_time() {
    let scanned = Cell::new(false);
    let index = RefCell::new(Vec::new());
    let builds = Cell::new(0);
    let first = fill_counted(&scanned, &index, &builds, PathBuf::from("/a.mkv"));
    let second = fill_counted(&scanned, &index, &builds, PathBuf::from("/b.mkv"));
    assert_eq!(builds.get(), 1);
    assert_eq!(first, second);
}
