//! Platform trash: **Freedesktop** [`$XDG_DATA_HOME/Trash`](https://specifications.freedesktop.org/trash-spec/trashspec-latest.html)
//! on typical Linux; **Finder** layout on macOS (`~/.Trash`, per-volume `.Trashes/<uid>`).
//! Used after [gio::File::trash] to find the moved file for session Undo.

use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::gio::prelude::FileExt;

use glib::GStr;

#[cfg(target_os = "macos")]
fn uid_dir() -> String {
    unsafe { libc::getuid() }.to_string()
}

#[cfg(target_os = "macos")]
fn macos_trash_root(original_before_trash: &Path) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let home_canon = std::fs::canonicalize(&home).unwrap_or(home);
    if original_before_trash.starts_with(&home_canon) {
        let t = home_canon.join(".Trash");
        return t.is_dir().then_some(t);
    }
    if let Ok(rest) = original_before_trash.strip_prefix("/Volumes") {
        if let Some(c) = rest.components().next() {
            let vol = Path::new("/Volumes").join(c.as_os_str());
            let t = vol.join(".Trashes").join(uid_dir());
            if t.is_dir() {
                return Some(t);
            }
        }
    }
    let t = home_canon.join(".Trash");
    t.is_dir().then_some(t)
}

#[cfg(target_os = "macos")]
fn find_macos_trashed_item(
    original_before_trash: &Path,
    size_bytes: Option<u64>,
) -> Option<PathBuf> {
    let dir = macos_trash_root(original_before_trash)?;
    let name = original_before_trash.file_name()?;
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    let entries = std::fs::read_dir(&dir).ok()?;
    for e in entries.filter_map(std::io::Result::ok) {
        let p = e.path();
        if p.file_name() != Some(name) {
            continue;
        }
        let md = e.metadata().ok()?;
        if !md.is_file() {
            continue;
        }
        if let Some(want) = size_bytes {
            if md.len() != want {
                continue;
            }
        }
        let t = md
            .modified()
            .ok()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let take = best.as_ref().map_or(true, |(_, tt)| t > *tt);
        if take {
            best = Some((p, t));
        }
    }
    best.map(|x| x.0)
}

#[cfg(target_os = "macos")]
fn is_macos_trash_item(p: &Path) -> bool {
    p.ancestors().any(|a| {
        a.file_name().is_some_and(|n| n == ".Trash" || n == ".Trashes")
    })
}

fn rename_cross_fs_ok(src: &Path, dst: &Path) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            std::fs::copy(src, dst)?;
            std::fs::remove_file(src)?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// `~/.local/share` (or `XDG_DATA_HOME`) with `…/Trash`.
fn trash_base() -> Option<PathBuf> {
    let d = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            let h = std::env::var_os("HOME")?;
            Some(PathBuf::from(h).join(".local/share"))
        })?;
    let b = d.join("Trash");
    let files = b.join("files");
    let info = b.join("info");
    if !files.is_dir() || !info.is_dir() {
        return None;
    }
    Some(b)
}

fn local_path_from_trashinfo_value(v: &str) -> Option<PathBuf> {
    let t = v.trim();
    if t.is_empty() {
        return None;
    }
    if t.starts_with("file:") {
        return gio::File::for_uri(t).path();
    }
    let dec = glib::uri_unescape_string(t, GStr::NONE)?;
    let s = dec.to_string();
    if s.starts_with('/') {
        return Some(PathBuf::from(s));
    }
    None
}

/// Value after the first `Path=` in trashinfo.
fn path_value_from_info(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Path=") {
            return Some(rest.to_string());
        }
        if let Some(rest) = t.strip_prefix("path=") {
            return Some(rest.to_string());
        }
    }
    None
}

/// Resolves the trashed **file** path after [gio::File::trash] so Undo can call [untrash_to_target].
///
/// Pass `size_bytes` from [std::fs::metadata]::len **before** trash on macOS (disambiguates same
/// basename). Linux XDG ignores `size_bytes`.
pub fn find_trash_files_stored_path(
    original_before_trash: &Path,
    size_bytes: Option<u64>,
) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    if let Some(p) = find_macos_trashed_item(original_before_trash, size_bytes) {
        return Some(p);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = size_bytes;

    let base = trash_base()?;
    let files_dir = base.join("files");
    let info_dir = base.join("info");
    let want =
        std::fs::canonicalize(original_before_trash).unwrap_or_else(|_| original_before_trash.to_path_buf());
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    let entries = std::fs::read_dir(&info_dir).ok()?;
    for e in entries.filter_map(std::io::Result::ok) {
        let ip = e.path();
        if ip.extension() != Some(std::ffi::OsStr::new("trashinfo")) {
            continue;
        }
        let s = std::fs::read_to_string(&ip).ok()?;
        let raw = path_value_from_info(&s)?;
        let p = local_path_from_trashinfo_value(&raw)?;
        if p != want {
            continue;
        }
        let stem = ip.file_stem()?;
        let in_files = files_dir.join(stem);
        if !in_files.is_file() {
            continue;
        }
        let t = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::UNIX_EPOCH);
        let take = best.as_ref().map_or(true, |(_, tt)| t > *tt);
        if take {
            best = Some((in_files, t));
        }
    }
    best.map(|(p, _)| p)
}

/// Move a file from Trash back to [target] and remove the corresponding `.trashinfo` when present.
pub fn untrash_to_target(in_trash: &Path, target: &Path) -> std::io::Result<()> {
    if let Some(p) = target.parent() {
        std::fs::create_dir_all(p)?;
    }

    #[cfg(target_os = "macos")]
    if is_macos_trash_item(in_trash) {
        return rename_cross_fs_ok(in_trash, target);
    }

    let (files_dir, info_dir) = {
        let b = trash_base()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no XDG trash"))?;
        (b.join("files"), b.join("info"))
    };
    if in_trash.parent() != Some(files_dir.as_path()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "trash: path is not in Trash/files",
        ));
    }
    let name = in_trash.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "trash: no file name")
    })?;
    let mut inf = name.to_string_lossy().into_owned();
    inf.push_str(".trashinfo");
    let info = info_dir.join(&inf);
    rename_cross_fs_ok(in_trash, target)?;
    if info.is_file() {
        let _ = std::fs::remove_file(info);
    }
    Ok(())
}
