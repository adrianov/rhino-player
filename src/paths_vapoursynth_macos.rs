// Homebrew **vapoursynth** R76+: **`libvsscript.dylib`** under `…/vapoursynth/`. mpv's **`vf=vapoursynth`**
// dlopen **`libvapoursynth-script.dylib`**. macOS dyld only honors **`DYLD_LIBRARY_PATH` at process start**
// — set it via a one-time **re-exec** ([`macos_reexec_for_vapoursynth_dyld_if_needed`]).

#[cfg(target_os = "macos")]
use std::ffi::{CString, OsString};

#[cfg(target_os = "macos")]
const VS_HOMEBREW_PREFIXES: &[&str] = &["/opt/homebrew", "/usr/local"];

#[cfg(target_os = "macos")]
const VSSCRIPT_DYLIB: &str = "libvsscript.dylib";

#[cfg(target_os = "macos")]
const MPV_VSSCRIPT_DYLIB: &str = "libvapoursynth-script.dylib";

#[cfg(target_os = "macos")]
const DYLD_PRIMED_VAR: &str = "RHINO_DYLD_PRIMED";

include!("paths_vapoursynth_macos_config.rs");

#[cfg(target_os = "macos")]
fn vsscript_dir_under_libexec(lib_root: &Path) -> Option<PathBuf> {
    if !lib_root.is_dir() {
        return None;
    }
    let py_dirs = std::fs::read_dir(lib_root).ok()?;
    for py in py_dirs.flatten() {
        if !py.file_name().to_string_lossy().starts_with("python") {
            continue;
        }
        if let Some(dir) = vsscript_dylib_in_python_dir(&py.path()) {
            return Some(dir);
        }
    }
    None
}

/// `…/site-packages/vapoursynth/` when it holds [`VSSCRIPT_DYLIB`].
#[cfg(target_os = "macos")]
fn vsscript_dylib_in_python_dir(py: &Path) -> Option<PathBuf> {
    let vs = py.join("site-packages/vapoursynth");
    if vs.join(VSSCRIPT_DYLIB).is_file() {
        return std::fs::canonicalize(&vs).ok().or(Some(vs));
    }
    None
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_vapoursynth_lib_dir() -> Option<PathBuf> {
    for prefix in VS_HOMEBREW_PREFIXES {
        let opt = Path::new(prefix).join("opt/vapoursynth/libexec/lib");
        if let Some(d) = vsscript_dir_under_libexec(&opt) {
            return Some(d);
        }
        let cellar = Path::new(prefix).join("Cellar/vapoursynth");
        if let Ok(vers) = std::fs::read_dir(&cellar) {
            for ver in vers.flatten() {
                if let Some(d) = vsscript_dir_under_libexec(&ver.path().join("libexec/lib")) {
                    return Some(d);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn dylib_alias_dir() -> PathBuf {
    crate::paths::app_config()
        .map(|c| c.join("dylib"))
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|h| h.join(".config/rhino/dylib"))
                .unwrap_or_else(|| std::env::temp_dir().join("rhino-player-dylib"))
        })
}

#[cfg(target_os = "macos")]
fn ensure_mpv_vsscript_alias(vs_lib: &Path) -> Option<PathBuf> {
    let vsscript = mpv_vsscript_target(vs_lib)?;
    if vs_lib.join(MPV_VSSCRIPT_DYLIB).is_file() {
        return None;
    }
    let alias_dir = dylib_alias_dir();
    std::fs::create_dir_all(&alias_dir).ok()?;
    let alias = alias_dir.join(MPV_VSSCRIPT_DYLIB);
    recreate_alias_symlink(&vsscript, &alias)?;
    Some(alias_dir)
}

/// The real `libvsscript.dylib` to alias from mpv's legacy name; logs when absent.
#[cfg(target_os = "macos")]
fn mpv_vsscript_target(vs_lib: &Path) -> Option<PathBuf> {
    let vsscript = vs_lib.join(VSSCRIPT_DYLIB);
    if !vsscript.is_file() {
        eprintln!(
            "[rhino] video: VapourSynth missing {VSSCRIPT_DYLIB} under {}",
            vs_lib.display()
        );
        return None;
    }
    Some(vsscript)
}

/// Replace any stale alias, then link `vsscript`; logs and fails soft on error.
#[cfg(target_os = "macos")]
fn recreate_alias_symlink(vsscript: &Path, alias: &Path) -> Option<()> {
    if alias.is_symlink() || alias.is_file() {
        let _ = std::fs::remove_file(alias);
    }
    std::os::unix::fs::symlink(vsscript, alias)
        .map_err(|e| {
            eprintln!(
                "[rhino] video: symlink {} -> {} failed: {e}",
                alias.display(),
                vsscript.display()
            )
        })
        .ok()
}

/// Build **`DYLD_LIBRARY_PATH`** entries for VapourSynth + the mpv legacy script dylib name.
#[cfg(target_os = "macos")]
fn macos_vapoursynth_dyld_paths() -> Option<String> {
    let vs_lib = macos_vapoursynth_lib_dir()?;
    let mut parts = Vec::new();
    if let Some(alias_dir) = ensure_mpv_vsscript_alias(&vs_lib) {
        parts.push(alias_dir);
    }
    parts.push(vs_lib);
    let merged = parts
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(":");
    Some(merged)
}

#[cfg(target_os = "macos")]
fn cstring_lossy(s: &std::ffi::OsStr) -> CString {
    use std::os::unix::ffi::OsStrExt;
    CString::new(s.as_bytes()).unwrap_or_else(|_| CString::new(b".").unwrap())
}

/// Re-exec this binary once so **`DYLD_LIBRARY_PATH`** is set before dyld loads anything for mpv.
#[cfg(target_os = "macos")]
pub fn macos_reexec_for_vapoursynth_dyld_if_needed() {
    if std::env::var_os(DYLD_PRIMED_VAR).is_some() {
        return;
    }
    // Before VSScript/mpv load: repair stale Cellar Python paths from `brew upgrade python`.
    macos_ensure_vapoursynth_python_config();
    let Some(add) = macos_vapoursynth_dyld_paths() else {
        eprintln!(
            "[rhino] video: VapourSynth not found — Smooth 60 needs `brew install vapoursynth vapoursynth-mvtools`"
        );
        return;
    };
    let dyld = merged_dyld_value(&add);
    let env = reexec_env(&dyld);
    execve_reexec(env);
}

include!("paths_vapoursynth_macos_reexec.rs");

#[cfg(all(test, target_os = "macos"))]
mod macos_vapoursynth_lib_tests {
    include!("paths_vapoursynth_macos_tests.rs");
}
