//! Bundled [Freedesktop icon theme] tree under `data/icons/` (see that directory’s README).
//!
//! [Freedesktop icon theme]: https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html

use std::path::PathBuf;

use crate::app::APP_ID;

/// Adds `CARGO_MANIFEST_DIR/data/icons` to the process icon search path so the app id icon
/// resolves from the build tree (e.g. `cargo run`) without a system install.
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
    theme.add_search_path(&dir);
    gtk::Window::set_default_icon_name(APP_ID);
}
