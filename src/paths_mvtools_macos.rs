// Homebrew **vapoursynth-mvtools** (formerly **mvtools**): plugin is **`mvtools.dylib`** under
// `…/site-packages/vapoursynth/plugins/`. Legacy installs used **`libmvtools.dylib`** in **`$(brew --prefix)/lib`**.

#[cfg(target_os = "macos")]
const HOMEBREW_PREFIXES: &[&str] = &["/opt/homebrew", "/usr/local"];

#[cfg(target_os = "macos")]
const MVTOOLS_PLUGIN_NAMES: &[&str] = &["mvtools.dylib", "libmvtools.dylib"];

#[cfg(target_os = "macos")]
fn canon_mvtools(p: PathBuf) -> PathBuf {
    std::fs::canonicalize(&p).unwrap_or(p)
}

#[cfg(target_os = "macos")]
fn mvtools_in_vapoursynth_plugins(lib_root: &Path) -> Option<PathBuf> {
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
        let plugins = py
            .path()
            .join("site-packages/vapoursynth/plugins");
        for name in MVTOOLS_PLUGIN_NAMES {
            let p = plugins.join(name);
            if p.is_file() {
                return Some(canon_mvtools(p));
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_mvtools_lib_search() -> Option<PathBuf> {
    for prefix in HOMEBREW_PREFIXES {
        let legacy = Path::new(prefix).join("lib/libmvtools.dylib");
        if legacy.is_file() {
            return Some(canon_mvtools(legacy));
        }
        let opt_lib = Path::new(prefix).join("opt/vapoursynth-mvtools/lib");
        if let Some(p) = mvtools_in_vapoursynth_plugins(&opt_lib) {
            return Some(p);
        }
        let cellar = Path::new(prefix).join("Cellar/vapoursynth-mvtools");
        if let Ok(vers) = std::fs::read_dir(&cellar) {
            for ver in vers.flatten() {
                if let Some(p) = mvtools_in_vapoursynth_plugins(&ver.path().join("lib")) {
                    return Some(p);
                }
            }
        }
    }
    None
}

#[cfg(all(test, target_os = "macos"))]
mod macos_mvtools_search_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn homebrew_vapoursynth_mvtools_if_installed() {
        if !Path::new("/opt/homebrew/opt/vapoursynth-mvtools").exists()
            && !Path::new("/usr/local/opt/vapoursynth-mvtools").exists()
        {
            return;
        }
        let hit = macos_mvtools_lib_search().expect("vapoursynth-mvtools installed but not found");
        let s = hit.to_string_lossy();
        assert!(
            s.ends_with("mvtools.dylib") || s.ends_with("libmvtools.dylib"),
            "unexpected plugin path: {s}"
        );
    }
}
