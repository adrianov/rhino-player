//! Bundled [Freedesktop icon theme] tree under `data/icons/` (see that directory’s README).
//!
//! [Freedesktop icon theme]: https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html

use crate::app::APP_ID;
use crate::paths;

/// Inserts **`.app` Resources**, **`PREFIX/share/rhino-player/icons`**, or **`data/icons`** at the
/// **front** of the icon search path so bundled **`hicolor`** entries (e.g. `speedometer-symbolic`)
/// win when the platform theme is incomplete (typical on macOS + Homebrew GTK).
pub fn register_hicolor_from_manifest() {
    let Some(dir) = paths::bundled_data_icons_dir_for_runtime() else {
        return;
    };
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let theme = gtk::IconTheme::for_display(&display);
    let mut search_path = theme.search_path();
    if !search_path.iter().any(|p| p == &dir) {
        search_path.insert(0, dir);
    }
    theme.set_search_path(
        &search_path
            .iter()
            .map(std::path::PathBuf::as_path)
            .collect::<Vec<_>>(),
    );
    gtk::Window::set_default_icon_name(APP_ID);
}
