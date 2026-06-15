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

#[cfg(target_os = "macos")]
fn vsscript_dir_under_libexec(lib_root: &Path) -> Option<PathBuf> {
    if !lib_root.is_dir() {
        return None;
    }
    let py_dirs = std::fs::read_dir(lib_root).ok()?;
    for py in py_dirs.flatten() {
        if !py
            .file_name()
            .to_string_lossy()
            .starts_with("python")
        {
            continue;
        }
        let vs = py
            .path()
            .join("site-packages/vapoursynth");
        if vs.join(VSSCRIPT_DYLIB).is_file() {
            return std::fs::canonicalize(&vs).ok().or(Some(vs));
        }
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
    let vsscript = vs_lib.join(VSSCRIPT_DYLIB);
    if !vsscript.is_file() {
        eprintln!("[rhino] video: VapourSynth missing {VSSCRIPT_DYLIB} under {}", vs_lib.display());
        return None;
    }
    if vs_lib.join(MPV_VSSCRIPT_DYLIB).is_file() {
        return None;
    }
    let alias_dir = dylib_alias_dir();
    std::fs::create_dir_all(&alias_dir).ok()?;
    let alias = alias_dir.join(MPV_VSSCRIPT_DYLIB);
    if alias.is_symlink() || alias.is_file() {
        let _ = std::fs::remove_file(&alias);
    }
    std::os::unix::fs::symlink(&vsscript, &alias).map_err(|e| {
        eprintln!(
            "[rhino] video: symlink {} -> {} failed: {e}",
            alias.display(),
            vsscript.display()
        );
    }).ok()?;
    Some(alias_dir)
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
    let Some(add) = macos_vapoursynth_dyld_paths() else {
        eprintln!(
            "[rhino] video: VapourSynth not found — Smooth 60 needs `brew install vapoursynth vapoursynth-mvtools`"
        );
        return;
    };
    let dyld = match std::env::var_os("DYLD_LIBRARY_PATH") {
        Some(cur) if !cur.is_empty() => format!("{add}:{}", cur.to_string_lossy()),
        _ => add,
    };
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[rhino] video: re-exec skipped (current_exe: {e})");
            return;
        }
    };
    let mut env: Vec<(OsString, OsString)> = std::env::vars_os().collect();
    env.retain(|(k, _)| k != "DYLD_LIBRARY_PATH");
    env.push(("DYLD_LIBRARY_PATH".into(), OsString::from(dyld)));
    env.push((DYLD_PRIMED_VAR.into(), "1".into()));

    let arg_c: Vec<CString> = std::env::args_os()
        .map(|a| cstring_lossy(a.as_os_str()))
        .collect();
    let mut arg_ptrs: Vec<*const libc::c_char> = arg_c.iter().map(|a| a.as_ptr()).collect();
    arg_ptrs.push(std::ptr::null());

    let env_c: Vec<CString> = env
        .iter()
        .map(|(k, v)| {
            use std::os::unix::ffi::OsStrExt;
            let mut pair = k.as_os_str().as_bytes().to_vec();
            pair.push(b'=');
            pair.extend_from_slice(v.as_os_str().as_bytes());
            CString::new(pair).unwrap_or_else(|_| CString::new(b"RHINO_DYLD_PRIMED=1").unwrap())
        })
        .collect();
    let mut env_ptrs: Vec<*const libc::c_char> = env_c.iter().map(|e| e.as_ptr()).collect();
    env_ptrs.push(std::ptr::null());

    eprintln!("[rhino] video: re-exec for VapourSynth (DYLD_LIBRARY_PATH at process start)");
    unsafe {
        libc::execve(
            cstring_lossy(exe.as_os_str()).as_ptr(),
            arg_ptrs.as_ptr(),
            env_ptrs.as_ptr(),
        );
    }
    eprintln!(
        "[rhino] video: re-exec failed: {}",
        std::io::Error::last_os_error()
    );
    std::process::exit(1);
}

#[cfg(all(test, target_os = "macos"))]
mod macos_vapoursynth_lib_tests {
    use super::*;

    #[test]
    fn homebrew_vapoursynth_lib_if_installed() {
        if !Path::new("/opt/homebrew/opt/vapoursynth").exists()
            && !Path::new("/usr/local/opt/vapoursynth").exists()
        {
            return;
        }
        let dir = macos_vapoursynth_lib_dir().expect("vapoursynth installed but lib dir missing");
        assert!(dir.join(VSSCRIPT_DYLIB).is_file());
        let dyld = macos_vapoursynth_dyld_paths().expect("dyld paths");
        assert!(!dyld.is_empty());
    }
}
