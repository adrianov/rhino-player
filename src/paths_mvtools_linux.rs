// Linux MVTools: distro `vapoursynth/` paths, pipx/vsrepo under `~/.local`, then a bounded walk.

#[cfg(not(target_os = "macos"))]
const MVTOOLS_FILE: &str = "libmvtools.so";

#[cfg(not(target_os = "macos"))]
const DISTRO_MVTOOLS_PATHS: &[&str] = &[
    "/usr/lib/x86_64-linux-gnu/vapoursynth/libmvtools.so",
    "/usr/lib/vapoursynth/libmvtools.so",
    "/usr/local/lib/vapoursynth/libmvtools.so",
];

#[cfg(not(target_os = "macos"))]
pub(crate) fn linux_mvtools_lib_search() -> Option<PathBuf> {
    for c in DISTRO_MVTOOLS_PATHS {
        let p = Path::new(c);
        if p.is_file() {
            return std::fs::canonicalize(p).ok().or(Some(p.to_path_buf()));
        }
    }
    extra_mvtools_search()
}

#[cfg(not(target_os = "macos"))]
fn extra_mvtools_search() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let local = PathBuf::from(home).join(".local");
    mvtools_in_pipx_venvs(&local)
        .or_else(|| find_file_breadth_first(&local, MVTOOLS_FILE, 14, 8000))
}

/// Pipx venvs under **`~/.local/share/pipx/venvs`**, **`~/.local/pipx/venvs`**, or **`$PIPX_HOME/venvs`**:
/// **`…/site-packages/vapoursynth/plugins/vsrepo/libmvtools.so`**.
#[cfg(not(target_os = "macos"))]
fn mvtools_scan_pipx_venvs_root(venvs_root: &Path) -> Option<PathBuf> {
    let venvs = std::fs::read_dir(venvs_root).ok()?;
    for venv in venvs.flatten() {
        let Ok(vft) = venv.file_type() else {
            continue;
        };
        if !vft.is_dir() {
            continue;
        }
        match mvtools_in_venv_lib(&venv.path()) {
            Some(Some(p)) => return Some(p),
            Some(None) => {}
            // Unreadable `lib/` aborts the whole scan, as the original `?` did.
            None => return None,
        }
    }
    None
}

/// Scan `<venv>/lib/python*/site-packages/vapoursynth/plugins/vsrepo/`.
/// Outer `None` = lib dir unreadable (abort); inner `None` = scanned, no hit.
#[cfg(not(target_os = "macos"))]
fn mvtools_in_venv_lib(venv: &Path) -> Option<Option<PathBuf>> {
    let pys = std::fs::read_dir(venv.join("lib")).ok()?;
    for py in pys.flatten() {
        if let Some(p) = vsrepo_mvtools_plugin(&py) {
            return Some(Some(p));
        }
    }
    Some(None)
}

/// `lib/python*/site-packages/vapoursynth/plugins/vsrepo/<MVTOOLS_FILE>` when present;
/// non-`python*` dirs and files are skipped.
#[cfg(not(target_os = "macos"))]
fn vsrepo_mvtools_plugin(py: &std::fs::DirEntry) -> Option<PathBuf> {
    let Ok(pft) = py.file_type() else {
        return None;
    };
    if !pft.is_dir() {
        return None;
    }
    if !py.file_name().to_string_lossy().starts_with("python") {
        return None;
    }
    let p = py
        .path()
        .join("site-packages/vapoursynth/plugins/vsrepo")
        .join(MVTOOLS_FILE);
    if p.is_file() {
        return Some(std::fs::canonicalize(&p).unwrap_or(p));
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn mvtools_in_pipx_venvs(local: &Path) -> Option<PathBuf> {
    mvtools_scan_pipx_venvs_root(&local.join("share/pipx/venvs"))
        .or_else(|| mvtools_scan_pipx_venvs_root(&local.join("pipx/venvs")))
        .or_else(|| {
            let ph = std::env::var_os("PIPX_HOME")?;
            mvtools_scan_pipx_venvs_root(&PathBuf::from(ph).join("venvs"))
        })
}

#[cfg(not(target_os = "macos"))]
/// Breadth-first search for `file_name` under `root`, at most `max_depth` directory levels from
/// `root`, stopping after `max_dir_reads` `read_dir` calls (avoids huge trees). Symlink directories
/// are not descended (same idea as Python `follow_symlinks=False`), so cycles do not burn the read budget.
fn find_file_breadth_first(
    root: &Path,
    file_name: &str,
    max_depth: u32,
    max_dir_reads: usize,
) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }
    use std::collections::VecDeque;
    let mut q: VecDeque<(PathBuf, u32)> = VecDeque::new();
    q.push_back((root.to_path_buf(), 0));
    let mut reads = 0usize;
    while let Some((dir, depth)) = q.pop_front() {
        if reads >= max_dir_reads {
            return None;
        }
        reads += 1;
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in read.flatten() {
            if let Some(hit) = walk_entry_for_file(e, depth, max_depth, file_name, &mut q) {
                return Some(hit);
            }
        }
    }
    None
}

/// Classify one directory entry during [`find_file_breadth_first`]: queue subdirectories for
/// descent and return the canonicalized path on a name hit (`None` = skip).
#[cfg(not(target_os = "macos"))]
fn walk_entry_for_file(
    e: std::fs::DirEntry,
    depth: u32,
    max_depth: u32,
    file_name: &str,
    q: &mut std::collections::VecDeque<(PathBuf, u32)>,
) -> Option<PathBuf> {
    let Ok(ft) = e.file_type() else {
        return None;
    };
    let p = e.path();
    if ft.is_dir() && !ft.is_symlink() {
        if depth < max_depth {
            q.push_back((p, depth + 1));
        }
        return None;
    }
    if p.file_name().is_some_and(|f| f == file_name) {
        return Some(std::fs::canonicalize(&p).unwrap_or(p));
    }
    None
}
