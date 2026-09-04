/// Real `temp_dir` layout for tests. [ScratchTmpOrder] avoids picking up unrelated videos when
/// `prev_before_current` / `next_after_eof` walk up to `/tmp`: **First** = no lexically earlier
/// peers scanned; **Last** = no later peers scanned.
#[derive(Clone, Copy)]
enum ScratchTmpOrder {
    First,
    Last,
}

fn scratch_island(label: &str, order: ScratchTmpOrder) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let prefix = match order {
        ScratchTmpOrder::First => "!rhino_sib",
        ScratchTmpOrder::Last => "zzz_rhino_sib",
    };
    let p = std::env::temp_dir().join(format!(
        "{}_{}_{}_{:?}_{}",
        prefix,
        label,
        std::process::id(),
        std::thread::current().id(),
        nanos
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn media_flat(island: &Path) -> PathBuf {
    let m = island.join("media");
    fs::create_dir_all(&m).unwrap();
    m
}

fn assert_same_path(got: &Path, want: &Path) {
    assert!(
        video_ext::paths_same_file(got, want),
        "got {} want {}",
        got.display(),
        want.display()
    );
}

/// Creates [path] as a directory.
fn ensure_dir(path: &Path) {
    fs::create_dir_all(path).unwrap();
}

/// Creates parent dir if needed, writes an empty video placeholder, returns the file path.
fn seeded_video(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    touch_file(&p);
    p
}

fn touch_file(path: &Path) {
    fs::write(path, b"x").unwrap();
}

/// Removes a scratch island, ignoring absence.
fn cleanup(island: &Path) {
    let _ = fs::remove_dir_all(island);
}
