//! Linux: embed `DT_RUNPATH` so a local **libmpv** in `/usr/local` is preferred without `LD_LIBRARY_PATH`.
//! macOS: Homebrew **opt/mpv/lib**, **Cellar/mpv/**/lib, then generic **lib** dirs for `-lmpv`.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    match std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() {
        Some("linux") => linux_runpath(),
        Some("macos") => macos_libmpv_search(),
        _ => {}
    }
}

fn linux_runpath() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").ok().and_then(|a| {
        let triplet = match a.as_str() {
            "x86_64" => Some("x86_64-linux-gnu"),
            "aarch64" => Some("aarch64-linux-gnu"),
            "arm" => Some("arm-linux-gnueabihf"),
            _ => None,
        };
        triplet.map(|t| format!("/usr/local/lib/{t}"))
    });
    for dir in [arch.as_deref(), Some("/usr/local/lib")]
        .into_iter()
        .flatten()
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
    }
    println!("cargo:rustc-link-arg=-Wl,--enable-new-dtags");
}

fn macos_libmpv_search() {
    for lib in homebrew_opt_mpv_lib_dirs()
        .into_iter()
        .chain(homebrew_cellar_mpv_lib_dirs())
        .chain(homebrew_lib_roots())
    {
        if mpv_dylib_present(&lib) {
            emit_mpv_link(&lib);
            return;
        }
    }
    let roots = homebrew_lib_roots();
    if roots.is_empty() {
        println!("cargo:warning=No Homebrew lib directory found. Install libmpv: `brew install mpv`.");
        return;
    }
    println!(
        "cargo:warning=libmpv not found ({:?}). Run `brew install mpv`.",
        roots
    );
}

fn emit_mpv_link(lib: &Path) {
    let dir = lib.display();
    println!("cargo:rustc-link-search=native={dir}");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
}

fn homebrew_opt_mpv_lib_dirs() -> Vec<PathBuf> {
    ["/opt/homebrew/opt/mpv/lib", "/usr/local/opt/mpv/lib"]
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

fn homebrew_cellar_mpv_lib_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for cellar in ["/opt/homebrew/Cellar/mpv", "/usr/local/Cellar/mpv"] {
        let Ok(read) = std::fs::read_dir(cellar) else {
            continue;
        };
        let mut vers: Vec<PathBuf> = read
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        vers.sort();
        if let Some(lib) = vers.last().map(|p| p.join("lib")).filter(|p| p.is_dir()) {
            out.push(lib);
        }
    }
    out
}

fn homebrew_lib_roots() -> Vec<PathBuf> {
    ["/opt/homebrew/lib", "/usr/local/lib"]
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

fn mpv_dylib_present(lib_dir: &Path) -> bool {
    let Ok(read) = std::fs::read_dir(lib_dir) else {
        return false;
    };
    for e in read.flatten() {
        let n = e.file_name();
        let s = n.to_string_lossy();
        if s.starts_with("libmpv.") && s.ends_with(".dylib") {
            return true;
        }
    }
    false
}
