/// Stores a stable absolute path for SQLite ([crate::db::VideoPrefs::mvtools_lib]).
fn mvt_path_to_store(p: &std::path::Path) -> String {
    p.canonicalize()
        .map(|c| c.to_string_lossy().into_owned())
        .unwrap_or_else(|_| p.to_string_lossy().into_owned())
}

fn adopt_mvtools(v: &mut crate::db::VideoPrefs, p: &std::path::Path, note: &str) {
    let s = mvt_path_to_store(p);
    if v.mvtools_lib != s {
        v.mvtools_lib = s;
        crate::db::save_video(v);
    }
    std::env::set_var(crate::paths::RHINO_MVTOOLS_LIB_VAR, &v.mvtools_lib);
    eprintln!("[rhino] video: libmvtools -> {} ({note})", v.mvtools_lib);
}

/// Resolves the **MVTools** plugin file and sets `RHINO_MVTOOLS_LIB`.
///
/// - **macOS**: env, then [crate::paths::mvtools_lib_search] (`.app` / config vendor / Homebrew+seed).
///   SQLite cache is **not** consulted first — sticky Cellar paths survive `brew upgrade` and
///   would skip seeding `~/.config/rhino/lib`.
/// - **Linux**: env → cached path if still a file → search.
fn apply_mvtools_env(v: &mut crate::db::VideoPrefs) -> bool {
    if let Some(p) = crate::paths::mvtools_from_env() {
        adopt_mvtools(v, &p, crate::paths::RHINO_MVTOOLS_LIB_VAR);
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(p) = crate::paths::mvtools_lib_search() {
            adopt_mvtools(v, &p, "app / config vendor / Homebrew");
            return true;
        }
        eprintln!(
            "[rhino] video: libmvtools not found; set {} or run `brew install vapoursynth-mvtools` \
             once (Rhino vendors a copy under ~/.config/rhino/lib). See `data/vs/README.md`.",
            crate::paths::RHINO_MVTOOLS_LIB_VAR
        );
        false
    }
    #[cfg(not(target_os = "macos"))]
    {
        let c = v.mvtools_lib.trim();
        if !c.is_empty() {
            if std::path::Path::new(c).is_file() {
                std::env::set_var(crate::paths::RHINO_MVTOOLS_LIB_VAR, c);
                eprintln!("[rhino] video: libmvtools -> {c} (cached in settings)");
                return true;
            }
            v.mvtools_lib.clear();
            crate::db::save_video(v);
        }
        if let Some(p) = crate::paths::mvtools_lib_search() {
            adopt_mvtools(v, &p, "search");
            true
        } else {
            eprintln!(
                "[rhino] video: libmvtools not found; set {} or install MVTools (vsrepo / distro \
                 package). See `data/vs/README.md`.",
                crate::paths::RHINO_MVTOOLS_LIB_VAR
            );
            false
        }
    }
}
