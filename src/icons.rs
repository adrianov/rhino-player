//! Bundled [Freedesktop icon theme] tree under `data/icons/` (see that directory’s README).
//!
//! [Freedesktop icon theme]: https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html

use std::path::PathBuf;

use crate::app::APP_ID;

/// Inserts `CARGO_MANIFEST_DIR/data/icons` at the **front** of the icon search path so bundled
/// **`hicolor`** entries (e.g. `speedometer-symbolic` for playback speed) win when the platform theme
/// is incomplete (typical on macOS + Homebrew GTK).
pub fn register_hicolor_from_manifest() {
    let dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "data", "icons"]
        .iter()
        .collect();
    if !dir.is_dir() {
        return;
    }
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let theme = gtk::IconTheme::for_display(&display);
    let mut paths = theme.search_path();
    if !paths.iter().any(|p| p == &dir) {
        paths.insert(0, dir);
    }
    let pref: Vec<&std::path::Path> = paths.iter().map(std::path::PathBuf::as_path).collect();
    theme.set_search_path(&pref);
    gtk::Window::set_default_icon_name(APP_ID);
}
