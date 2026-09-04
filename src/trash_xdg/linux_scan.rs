use std::path::{Path, PathBuf};

/// Resolves the trashed **file** path after [gio::File::trash] so Undo can call
/// [super::untrash_to_target].
pub(super) fn find_trash_files_stored_path(
    original_before_trash: &Path,
    _size_bytes: Option<u64>,
) -> Option<PathBuf> {
    let base = trash_base()?;
    let files_dir = base.join("files");
    let info_dir = base.join("info");
    let want = canonical_or_self(original_before_trash);
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    for e in std::fs::read_dir(&info_dir)
        .ok()?
        .filter_map(std::io::Result::ok)
    {
        keep_newer(
            &mut best,
            scan_trashinfo_entry(&e, files_dir.as_path(), &want),
        )?;
    }
    best.map(|(p, _)| p)
}
