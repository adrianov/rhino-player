// Catalog forget-on-miss / adopt (feature 34). Sole policy owner for "path absent on disk".

/// Path absent on disk: adopt a finished incomplete-download sibling when possible; otherwise
/// drop continue history and the files catalog.
/// Returns the adopted path when rekey succeeds. No-op for paths that still exist or optical /
/// VIDEO_TS `.vob` identities.
pub(crate) fn forget_missing(path: &Path) -> Option<PathBuf> {
    forget_missing_paths(&[path.to_path_buf()])
        .into_iter()
        .next()
}

/// Pack many missing-path forgets: one log line, one SQLite transaction when dropping.
/// Returns adopted finished-download paths (same order as successful adopts).
pub(crate) fn forget_missing_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut adopted = Vec::new();
    let mut gone = Vec::new();
    for path in paths {
        if path.exists() || should_keep_missing(path) {
            continue;
        }
        if let Some(finished) = adopt_finished_download(path) {
            adopted.push(finished);
            continue;
        }
        gone.push(path.clone());
    }
    if !gone.is_empty() {
        log_drop_missing(&gone);
        drop_catalog_paths(&gone);
    }
    adopted
}

fn log_drop_missing(gone: &[PathBuf]) {
    if gone.len() == 1 {
        eprintln!("[rhino] catalog: drop missing {}", gone[0].display());
        return;
    }
    eprintln!(
        "[rhino] catalog: drop missing n={} first={}",
        gone.len(),
        gone[0].display()
    );
}

/// Open preflight for search / Lucky: true when a load may proceed.
/// Missing paths leave the catalog (via [forget_missing]); hollow/empty stay listed but unopenable.
pub(crate) fn path_is_openable(path: &Path) -> bool {
    match crate::media_open_fail::preflight_user_message(path) {
        None => true,
        Some(crate::media_open_fail::msg::MISSING) => {
            let _ = forget_missing(path);
            false
        }
        Some(_) => false,
    }
}

/// User-initiated open failed: forget a missing path, else drop continue for hollow/unreadable.
pub(crate) fn drop_after_open_fail(path: &Path, msg: &str) {
    if !crate::media_open_fail::should_drop_from_continue(msg) {
        return;
    }
    if msg == crate::media_open_fail::msg::MISSING {
        let _ = forget_missing(path);
    } else {
        remove_continue_entry(path);
    }
}

/// Continue-card Remove: forget catalog when the file is gone; else clear continue + undo snapshot.
pub(crate) fn dismiss_continue_path(path: &Path) -> Option<ListRemoveUndo> {
    if !path.exists() {
        let _ = forget_missing(path);
        return None;
    }
    let snap = capture_list_remove_undo(path);
    remove_continue_entry(path);
    Some(snap)
}

fn adopt_finished_download(path: &Path) -> Option<PathBuf> {
    let finished = crate::human_media_title::finished_download_path(path)?;
    if !crate::db::rekey_continue_path(path, &finished) {
        eprintln!(
            "[rhino] history: could not adopt finished download {} -> {}",
            path.display(),
            finished.display()
        );
        return None;
    }
    crate::db::ensure_file(&finished);
    // Stale `files` row for the incomplete name (rekey only moves history/media).
    crate::db::forget_file(path);
    eprintln!(
        "[rhino] history: incomplete download finished {} -> {}",
        path.display(),
        finished.display()
    );
    Some(finished)
}

fn drop_catalog_path(path: &Path) {
    drop_catalog_paths(&[path.to_path_buf()]);
}

fn drop_catalog_paths(paths: &[PathBuf]) {
    // Packed delete covers files/history/media keys. DVD title sets still need chapter-row purge.
    for path in paths {
        if video_ts_vob_name(path) || crate::video_ext::is_optical_disc_path(path) {
            crate::db::remove_history_matching_entity(path);
            crate::playback_entity::purge_extra_db_rows(path);
        }
    }
    crate::db::forget_files(paths);
}

fn should_forget_unparseable(path: &Path) -> bool {
    !crate::human_media_title::is_incomplete_download_path(path)
        && !crate::video_ext::is_optical_disc_path(path)
        && !video_ts_vob_name(path)
}

fn should_keep_missing(path: &Path) -> bool {
    crate::video_ext::is_optical_disc_path(path) || video_ts_vob_name(path)
}

/// Parent is `VIDEO_TS` and the name is a `.vob` (file need not exist — tests / gone chapters).
fn video_ts_vob_name(path: &Path) -> bool {
    path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("vob"))
        && path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("VIDEO_TS"))
}
