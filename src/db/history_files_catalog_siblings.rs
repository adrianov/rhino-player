//! Shallow sibling catalog registration on open-to-play (feature 34).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use super::super::history_key;
use super::{unix_now, with_files_conn, with_immediate_tx};

static FILES_EPOCH: AtomicU64 = AtomicU64::new(1);
static SESSION_SCANNED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Bumps when open-sibling registration inserts new catalog rows (search index reload).
pub fn files_catalog_epoch() -> u64 {
    FILES_EPOCH.load(Ordering::Relaxed)
}

fn bump_files_epoch() {
    FILES_EPOCH.fetch_add(1, Ordering::Relaxed);
}

/// Once per session per opened path key: register same-folder + sibling-folder videos (shallow).
/// Walk runs on a worker so open-to-play stays responsive.
pub fn ensure_open_siblings(opened: &Path) {
    let Some(key) = history_key(opened) else {
        return;
    };
    if !session_mark_scanned(&key) {
        return;
    }
    let opened = opened.to_path_buf();
    if let Err(e) = std::thread::Builder::new()
        .name("rhino-catalog-sib".into())
        .spawn(move || scan_and_register(&opened))
    {
        session_unmark(&key);
        eprintln!("[rhino] catalog: sibling scan spawn: {e}");
    }
}

fn scan_and_register(opened: &Path) {
    let paths = sibling_videos_shallow(opened);
    let n = paths.len();
    // Canonicalize before taking the DB lock so a large folder walk does not stall UI writers.
    let keys: Vec<String> = paths.iter().filter_map(|p| history_key(p)).collect();
    let inserted = ensure_keys_batch(&keys);
    if inserted > 0 {
        bump_files_epoch();
    }
    eprintln!(
        "[rhino] catalog: sibling scan n={n} keys={} inserted={inserted} for {}",
        keys.len(),
        opened.display()
    );
}

fn session_mark_scanned(key: &str) -> bool {
    let mut g = SESSION_SCANNED.lock().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(HashSet::new).insert(key.to_string())
}

fn session_unmark(key: &str) {
    let mut g = SESSION_SCANNED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(set) = g.as_mut() {
        set.remove(key);
    }
}

/// Videos in `opened`'s folder, plus videos directly in peer folders under the same parent.
/// No walk-up; no recurse into peer folders; skip peer scan when the parent is a filesystem root
/// (and on macOS, when the parent is `/Volumes`).
pub(crate) fn sibling_videos_shallow(opened: &Path) -> Vec<PathBuf> {
    let Some(dir) = opened.parent() else {
        return Vec::new();
    };
    let mut out = crate::video_ext::list_videos_in_dir(dir).unwrap_or_default();
    let Some(parent) = dir.parent() else {
        return out;
    };
    if skip_peer_dirs(parent) {
        return out;
    }
    for sdir in peer_dirs(parent) {
        if crate::video_ext::paths_same_file(&sdir, dir) {
            continue;
        }
        if let Some(v) = crate::video_ext::list_videos_in_dir(&sdir) {
            out.extend(v);
        }
    }
    out
}

/// Peer folders under a filesystem root (or macOS `/Volumes`) would scan unrelated mounts.
fn skip_peer_dirs(parent: &Path) -> bool {
    if parent.parent().is_none() {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        if parent == Path::new("/Volumes") {
            return true;
        }
    }
    false
}

fn peer_dirs(parent: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    rd.filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

fn ensure_keys_batch(keys: &[String]) -> usize {
    if keys.is_empty() {
        return 0;
    }
    let now = unix_now();
    with_files_conn(|c| {
        let mut inserted = 0usize;
        with_immediate_tx(c, |c| {
            for key in keys {
                inserted += c.execute(
                    "INSERT OR IGNORE INTO files (path, discovered_at) VALUES (?1, ?2)",
                    rusqlite::params![key, now],
                )?;
            }
            Ok(())
        })?;
        Ok(inserted)
    })
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "rhino-cat-sib-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, b"x").unwrap();
        p
    }

    fn names_of(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .filter_map(|p| p.file_name()?.to_str().map(str::to_string))
            .collect()
    }

    fn mkdir(p: &Path) {
        fs::create_dir_all(p).unwrap();
    }

    fn peer_tree() -> (PathBuf, PathBuf) {
        let root = scratch("peer");
        let a = root.join("show-a");
        let b = root.join("show-b");
        mkdir(&a);
        mkdir(&b);
        let opened = touch(&a, "ep1.mkv");
        touch(&a, "ep2.mkv");
        touch(&b, "ep1.mkv");
        mkdir(&b.join("nested"));
        touch(&b.join("nested"), "hidden.mkv");
        (root, opened)
    }

    #[test]
    fn same_folder_and_sibling_folder_shallow() {
        let (root, opened) = peer_tree();
        let names = names_of(&sibling_videos_shallow(&opened));
        assert_eq!(names.iter().filter(|n| *n == "ep1.mkv").count(), 2);
        assert!(names.contains(&"ep2.mkv".into()));
        assert!(!names.contains(&"hidden.mkv".into()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn does_not_walk_up_to_aunt_folder() {
        let root = scratch("aunt");
        let parent = root.join("parent");
        let aunt = root.join("aunt");
        let child = parent.join("child");
        mkdir(&child);
        mkdir(&aunt);
        let opened = touch(&child, "me.mkv");
        touch(&aunt, "other.mkv");

        let got = sibling_videos_shallow(&opened);
        assert!(
            got.iter()
                .all(|p| p.file_name().and_then(|n| n.to_str()) != Some("other.mkv"))
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn skip_peer_dirs_at_fs_root() {
        #[cfg(unix)]
        assert!(skip_peer_dirs(Path::new("/")));
        #[cfg(windows)]
        assert!(skip_peer_dirs(Path::new(r"C:\")));
        #[cfg(target_os = "macos")]
        assert!(skip_peer_dirs(Path::new("/Volumes")));
        assert!(!skip_peer_dirs(Path::new("/home/user/Shows")));
    }

    #[test]
    fn non_root_parent_includes_peer_folder_videos() {
        let root = scratch("fsroot");
        let a = root.join("only");
        mkdir(&a);
        let opened = touch(&a, "a.mkv");
        touch(&a, "b.mkv");
        let peer = root.join("peer");
        mkdir(&peer);
        touch(&peer, "c.mkv");
        let got = sibling_videos_shallow(&opened);
        assert!(got
            .iter()
            .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("c.mkv")));
        let _ = fs::remove_dir_all(&root);
    }
}
