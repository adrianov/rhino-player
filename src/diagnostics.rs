//! CLI **`--diagnostics`** / **`-D`**: Smooth Video status report (no UI).
//! See `docs/features/35-system-diagnostics.md`.

/// `--version` / `-V` before any macOS DYLD re-exec.
pub fn cli_version_exit() -> Option<i32> {
    if std::env::args()
        .skip(1)
        .any(|a| matches!(a.as_str(), "--version" | "-V"))
    {
        println!("rhino-player {}", env!("CARGO_PKG_VERSION"));
        return Some(0);
    }
    None
}

/// `--diagnostics` / `-D` after the macOS VSScript DYLD re-exec (when used).
pub fn cli_diagnostics_exit() -> Option<i32> {
    if std::env::args()
        .skip(1)
        .any(|a| matches!(a.as_str(), "--diagnostics" | "-D"))
    {
        return Some(run());
    }
    None
}

fn run() -> i32 {
    println!("Rhino Player diagnostics {}", env!("CARGO_PKG_VERSION"));
    print_identity();
    let ok = required_checks_ok();
    #[cfg(target_os = "macos")]
    print_macos_dyld();
    smooth_verdict(ok)
}

fn required_checks_ok() -> bool {
    [
        ("libmpv", probe_libmpv()),
        ("vapoursynth filter", probe_vapoursynth_filter()),
        ("VSScript", crate::video_pref::diagnose_vsscript()),
        ("MVTools", probe_mvtools()),
        ("bundled .vpy", probe_bundled_vpy()),
    ]
    .into_iter()
    .fold(true, |acc, (name, r)| acc & check_line(name, r))
}

fn smooth_verdict(ok: bool) -> i32 {
    if ok {
        println!("\nSmooth Video prerequisites: OK");
        0
    } else {
        println!("\nSmooth Video prerequisites: FAIL");
        println!("See Preferences → Smooth setup text, or docs/features/26-sixty-fps-motion.md");
        1
    }
}

fn check_line(name: &str, r: Result<String, String>) -> bool {
    match r {
        Ok(detail) => {
            println!("ok   {name}: {detail}");
            true
        }
        Err(detail) => {
            println!("FAIL {name}: {detail}");
            false
        }
    }
}

fn print_identity() {
    match std::env::current_exe() {
        Ok(p) => println!("binary: {}", p.display()),
        Err(e) => println!("binary: (unavailable: {e})"),
    }
    println!("os: {}", std::env::consts::OS);
}

fn probe_mvtools() -> Result<String, String> {
    if let Some(p) = crate::paths::mvtools_from_env() {
        return Ok(format!("RHINO_MVTOOLS_LIB={}", p.display()));
    }
    match crate::paths::mvtools_lib_search() {
        Some(p) => Ok(p.display().to_string()),
        None => {
            Err("not found (macOS: brew vapoursynth-mvtools / .app vendor; Linux: vsrepo)".into())
        }
    }
}

fn probe_bundled_vpy() -> Result<String, String> {
    match crate::paths::bundled_mvtools_60() {
        Some(p) => Ok(p.display().to_string()),
        None => Err("rhino_60_mvtools.vpy not found next to binary or in share/".into()),
    }
}

#[cfg(target_os = "macos")]
fn print_macos_dyld() {
    println!(
        "info macOS DYLD primed: {}",
        if std::env::var_os("RHINO_DYLD_PRIMED").is_some() {
            "yes"
        } else {
            "no"
        }
    );
    if let Some(dir) = crate::paths::macos_vapoursynth_lib_dir() {
        println!("info VSScript dir: {}", dir.display());
    }
}

include!("diagnostics/vf_probe.rs");
