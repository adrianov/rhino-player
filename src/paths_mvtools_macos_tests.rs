use super::*;
use std::path::Path;

#[test]
fn homebrew_vapoursynth_mvtools_if_installed() {
    if !Path::new("/opt/homebrew/opt/vapoursynth-mvtools").exists()
        && !Path::new("/usr/local/opt/vapoursynth-mvtools").exists()
    {
        return;
    }
    let hit = macos_homebrew_mvtools().expect("vapoursynth-mvtools installed but not found");
    let s = hit.to_string_lossy();
    assert!(
        s.ends_with("mvtools.dylib") || s.ends_with("libmvtools.dylib"),
        "unexpected plugin path: {s}"
    );
}

#[test]
fn search_prefers_stable_or_homebrew() {
    let hit = macos_mvtools_lib_search();
    if Path::new("/opt/homebrew/opt/vapoursynth-mvtools").exists()
        || Path::new("/usr/local/opt/vapoursynth-mvtools").exists()
        || macos_stable_mvtools().is_some()
    {
        assert!(hit.is_some());
    }
}

#[test]
fn search_prefers_config_vendor_over_cellar() {
    let Some(stable) = macos_config_mvtools() else {
        return;
    };
    let hit = macos_mvtools_lib_search().expect("config vendor present");
    assert_eq!(hit, stable);
    assert!(
        !hit.to_string_lossy().contains("/Cellar/"),
        "must not stick to Homebrew Cellar: {}",
        hit.display()
    );
}
