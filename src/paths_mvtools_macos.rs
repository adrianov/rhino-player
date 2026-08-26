// macOS MVTools: prefer a **stable** copy (`.app` Resources or `~/.config/rhino/lib`) over
// Homebrew Cellar / `python*` layout paths that break on `brew upgrade`. Packaging runs
// `scripts/macos-vendor-smooth-libs.sh`; first Homebrew hit also seeds the config tree.

#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "macos")]
const HOMEBREW_PREFIXES: &[&str] = &["/opt/homebrew", "/usr/local"];

#[cfg(target_os = "macos")]
const MVTOOLS_PLUGIN_NAMES: &[&str] = &["mvtools.dylib", "libmvtools.dylib"];

#[cfg(target_os = "macos")]
const BUNDLED_PLUGIN_REL: &str = "lib/vapoursynth/plugins/mvtools.dylib";

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
        if !py.file_name().to_string_lossy().starts_with("python") {
            continue;
        }
        if let Some(p) = mvtools_plugins_in_python_dir(&py.path()) {
            return Some(p);
        }
    }
    None
}

/// First MVTools dylib under `<python-dir>/site-packages/vapoursynth/plugins/`.
#[cfg(target_os = "macos")]
fn mvtools_plugins_in_python_dir(py: &Path) -> Option<PathBuf> {
    let plugins = py.join("site-packages/vapoursynth/plugins");
    MVTOOLS_PLUGIN_NAMES.iter().find_map(|name| {
        let p = plugins.join(name);
        p.is_file().then(|| canon_mvtools(p))
    })
}

/// `Contents/Resources/lib/vapoursynth/plugins/mvtools.dylib` inside a running `.app`.
#[cfg(target_os = "macos")]
pub(crate) fn macos_bundled_app_mvtools() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bin_dir = exe.parent()?;
    let contents = macos_app_contents_from_exe(bin_dir)?;
    let p = contents.join("Resources").join(BUNDLED_PLUGIN_REL);
    p.is_file().then(|| canon_mvtools(p))
}

/// `~/.config/rhino/lib/vapoursynth/plugins/mvtools.dylib` (seeded once from Homebrew).
#[cfg(target_os = "macos")]
pub(crate) fn macos_config_mvtools() -> Option<PathBuf> {
    let p = crate::paths::app_config()?.join(BUNDLED_PLUGIN_REL);
    p.is_file().then(|| canon_mvtools(p))
}

#[cfg(target_os = "macos")]
fn macos_homebrew_mvtools() -> Option<PathBuf> {
    for prefix in HOMEBREW_PREFIXES {
        let legacy = Path::new(prefix).join("lib/libmvtools.dylib");
        if legacy.is_file() {
            return Some(canon_mvtools(legacy));
        }
        let opt_lib = Path::new(prefix).join("opt/vapoursynth-mvtools/lib");
        if let Some(p) = mvtools_in_vapoursynth_plugins(&opt_lib) {
            return Some(p);
        }
        if let Some(p) = homebrew_cellar_mvtools(prefix) {
            return Some(p);
        }
    }
    None
}

/// Versioned `Cellar/vapoursynth-mvtools/<ver>/lib` layout.
#[cfg(target_os = "macos")]
fn homebrew_cellar_mvtools(prefix: &str) -> Option<PathBuf> {
    let cellar = Path::new(prefix).join("Cellar/vapoursynth-mvtools");
    let vers = std::fs::read_dir(cellar).ok()?;
    vers.flatten()
        .find_map(|ver| mvtools_in_vapoursynth_plugins(&ver.path().join("lib")))
}

/// Copy Homebrew MVTools into the user config tree (Homebrew-path-free `@loader_path` deps).
#[cfg(target_os = "macos")]
fn macos_seed_config_mvtools() -> Option<PathBuf> {
    if let Some(existing) = macos_config_mvtools() {
        return Some(existing);
    }
    let dest = crate::paths::app_config()?.join("lib/vapoursynth");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/macos-vendor-smooth-libs.sh");
    if !script.is_file() {
        eprintln!(
            "[rhino] video: cannot seed config MVTools (missing {})",
            script.display()
        );
        return None;
    }
    seed_status_result(Command::new(&script).arg(&dest).status(), &dest)
}

#[cfg(target_os = "macos")]
fn seed_status_result(
    status: std::io::Result<std::process::ExitStatus>,
    dest: &Path,
) -> Option<PathBuf> {
    match status {
        Ok(s) if s.success() => macos_config_mvtools().or_else(|| {
            eprintln!(
                "[rhino] video: seed script ok but {} missing",
                dest.join("plugins/mvtools.dylib").display()
            );
            None
        }),
        Ok(s) => {
            eprintln!(
                "[rhino] video: seed config MVTools failed (exit {})",
                s.code().unwrap_or(-1)
            );
            None
        }
        Err(e) => {
            eprintln!("[rhino] video: seed config MVTools failed: {e}");
            None
        }
    }
}

/// Stable copies only (`.app` Resources, then config vendor). No Homebrew.
#[cfg(target_os = "macos")]
pub(crate) fn macos_stable_mvtools() -> Option<PathBuf> {
    macos_bundled_app_mvtools().or_else(macos_config_mvtools)
}

/// True when Smooth can resolve MVTools without mutating the config tree (no seed side effect).
#[cfg(target_os = "macos")]
pub(crate) fn macos_mvtools_available() -> bool {
    macos_stable_mvtools().is_some() || macos_homebrew_mvtools().is_some()
}

/// Search order: **`.app` bundle** → **config vendor** → Homebrew (and seed config on first hit).
#[cfg(target_os = "macos")]
pub(crate) fn macos_mvtools_lib_search() -> Option<PathBuf> {
    if let Some(p) = macos_stable_mvtools() {
        return Some(p);
    }
    let brew = macos_homebrew_mvtools()?;
    if let Some(seeded) = macos_seed_config_mvtools() {
        eprintln!(
            "[rhino] video: seeded config MVTools -> {} (Homebrew was {})",
            seeded.display(),
            brew.display()
        );
        return Some(seeded);
    }
    Some(brew)
}

#[cfg(all(test, target_os = "macos"))]
mod macos_mvtools_search_tests {
    include!("paths_mvtools_macos_tests.rs");
}
