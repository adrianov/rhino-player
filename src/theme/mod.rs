//! App look: shell chrome, continue grid, pointer cursors, macOS hybrid overlays.
//! Dark style comes from [adw::StyleManager::set_color_scheme] in `app.rs` — do not set
//! `gtk-application-prefer-dark-theme` (unsupported with libadwaita).

mod cursor;

/// Main-window chrome: header, transport bar, seek/volume scales, header popovers.
/// Continue-grid and macOS overlay sheets stay in sibling stylesheets.
pub(crate) struct ShellChrome;

impl ShellChrome {
    fn stylesheet() -> &'static str {
        include_str!("shell.css")
    }
}

/// Continue strip: cards, Open tile, undo pill, progress styling ([`crate::recent_view`]).
const CONTINUE_GRID_CSS: &str = include_str!("continue_grid.css");

/// macOS hybrid render: window + video stack transparent so the native mpv layer shows
/// through the GLArea. Continue grid and toolbar chrome keep their own opaque backgrounds.
const MACOS_TRANSPARENT_CONTENT_CSS: &str = include_str!("macos_transparent.css");

/// Bottom transport chrome on gdk-macos (USER priority — wins over transparent window rules).
#[cfg(target_os = "macos")]
const MACOS_BOTTOM_CHROME_CSS: &str = include_str!("macos_bottom.css");
#[cfg(target_os = "macos")]
const MACOS_NATIVE_LISTS_CSS: &str = include_str!("macos_native_lists.css");

pub fn apply() {
    let shell = ShellChrome::stylesheet();
    let mut css = String::with_capacity(
        shell.len()
            + CONTINUE_GRID_CSS.len()
            + 256
            + if cfg!(target_os = "macos") {
                MACOS_TRANSPARENT_CONTENT_CSS.len()
            } else {
                0
            }
            + 8,
    );
    css.push_str(shell);
    css.push_str(CONTINUE_GRID_CSS);
    if cfg!(target_os = "macos") {
        css.push_str(MACOS_TRANSPARENT_CONTENT_CSS);
    }
    cursor::append_cursor_css(&mut css);
    let p = gtk::CssProvider::new();
    p.load_from_string(&css);
    if let Some(d) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &d,
            &p,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        #[cfg(target_os = "macos")]
        {
            let mut user = MACOS_BOTTOM_CHROME_CSS.to_string();
            user.push_str(include_str!("macos_header_compact.css"));
            user.push_str(MACOS_NATIVE_LISTS_CSS);
            let chrome = gtk::CssProvider::new();
            chrome.load_from_string(&user);
            gtk::style_context_add_provider_for_display(
                &d,
                &chrome,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
    }
    // GTK's default title-bar double-click toggles maximize after our HeaderBar gesture runs,
    // undoing `maximize()` before `fullscreen()` (first double-click looks maximized then snaps small).
    // Enter / menu use `toggle_fullscreen` only; disabling GDK's built-in action keeps parity.
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_titlebar_double_click(Some("none"));
        // GtkScale / GtkRange: primary click jumps the slider under the pointer (volume + seek).
        settings.set_gtk_primary_button_warps_slider(true);
    }
}
