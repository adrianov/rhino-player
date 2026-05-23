/// Extra styling on top of libadwaita: opaque content area, custom seek / time look, pointer on controls.
/// Dark style comes from [adw::StyleManager::set_color_scheme] in `app.rs` — do not set
/// `gtk-application-prefer-dark-theme` (unsupported with libadwaita).
const APP_CSS: &str = r#"
        window.rp-win {
            background-color: #242424;
            color: #eeeeec;
        }
        .rpb-header {
            background-color: #2d2d2d;
            color: #eeeeec;
            box-shadow: none;
            border: none;
            min-height: 0;
            padding: 0 4px;
        }
        toolbarview.rp-toolbar headerbar.rpb-header {
            min-height: 0;
            padding: 0 4px;
        }
        .rpb-header menubutton.flat > button {
            min-height: 26px;
            min-width: 26px;
            padding: 2px;
        }
        .rpb-header menubutton.flat image,
        .rpb-header button.rp-smooth-mbtn.flat image,
        .rpb-header button.rp-blackout-mbtn.flat image {
            color: #eeeeec;
            -gtk-icon-style: symbolic;
        }
        menubutton.rp-speed-mbtn.flat > button {
            min-height: 26px;
            min-width: 0;
            padding-left: 4px;
            padding-right: 4px;
        }
        .rpb-header windowcontrols button.image-button {
            min-height: 26px;
            min-width: 26px;
            padding: 1px;
        }
        label.rp-fs-clock {
            color: #c0bfbc;
            font-family: monospace, monospace;
            font-feature-settings: "tnum";
            margin-right: 6px;
            font-size: 0.92em;
        }
        menubutton.rp-speed-mbtn.flat box.rp-speed-face {
            padding: 0;
            margin: 0;
            border-spacing: 0;
        }
        menubutton.rp-speed-mbtn.flat label.rp-speed-readout {
            color: #9a9996;
            font-feature-settings: "tnum";
            font-size: 0.75em;
            padding: 0;
            margin: 0;
            min-width: 0;
            opacity: 0.92;
        }
        button.rp-smooth-mbtn.flat {
            min-height: 26px;
            min-width: 0;
            padding-left: 4px;
            padding-right: 4px;
        }
        button.rp-smooth-mbtn.flat box.rp-smooth-face {
            padding: 0;
            margin: 0;
            border-spacing: 0;
        }
        button.rp-smooth-mbtn.flat label.rp-smooth-readout {
            color: #9a9996;
            font-feature-settings: "tnum";
            font-size: 0.75em;
            padding: 0;
            margin: 0;
            min-width: 0;
            opacity: 0.92;
        }
        button.rp-blackout-mbtn.flat {
            min-height: 26px;
            min-width: 0;
            padding-left: 4px;
            padding-right: 4px;
        }
        button.rp-blackout-mbtn.flat box.rp-blackout-face {
            padding: 0;
            margin: 0;
            border-spacing: 0;
        }
        button.rp-blackout-mbtn.flat label.rp-blackout-readout {
            color: #9a9996;
            font-feature-settings: "tnum";
            font-size: 0.75em;
            padding: 0;
            margin: 0;
            min-width: 0;
            opacity: 0.92;
        }
        menubutton.rp-vol-mbtn.flat > button {
            min-height: 26px;
            min-width: 0;
            padding-left: 4px;
            padding-right: 4px;
        }
        menubutton.rp-vol-mbtn.flat box.rp-vol-face {
            padding: 0;
            margin: 0;
            border-spacing: 0;
        }
        menubutton.rp-vol-mbtn.flat label.rp-vol-readout {
            color: #9a9996;
            font-feature-settings: "tnum";
            font-size: 0.75em;
            padding: 0;
            margin: 0;
            min-width: 0;
            opacity: 0.92;
        }
        menubutton.rp-sub-mbtn.flat > button {
            min-height: 26px;
            min-width: 0;
            padding-left: 4px;
            padding-right: 4px;
        }
        menubutton.rp-sub-mbtn.flat box.rp-sub-face {
            padding: 0;
            margin: 0;
            border-spacing: 0;
        }
        menubutton.rp-sub-mbtn.flat label.rp-sub-readout {
            color: #9a9996;
            font-feature-settings: "tnum";
            font-size: 0.75em;
            padding: 0;
            margin: 0;
            min-width: 0;
            opacity: 0.92;
        }
        .rp-stack {
            background-color: #242424;
        }
        .rp-gl { background: #000000; min-height: 120px; }
        /* macOS native render: GLArea publishes alpha=0 pixels (cleared in
           connect_render) so the underlying CAOpenGLLayer (mpv video) shows through. */
        .rp-gl.rp-gl-native { background: transparent; background-color: transparent; }
        .rp-bottom {
            background-color: #2d2d2d;
            color: #eeeeec;
            box-shadow: none;
            border: none;
            padding: 2px 4px;
        }
        .rp-bottom button.flat {
            min-height: 26px;
            min-width: 26px;
            padding: 2px;
        }
        .rp-bottom button.flat image {
            color: #eeeeec;
            -gtk-icon-style: symbolic;
        }
        /* LTR clock spacing; avoid margin-inline-end (not in all GTK CSS parsers). */
        .rp-bottom .rpb-prev { margin-right: 1px; }
        .rp-bottom .rpb-play { margin-left: 1px; margin-right: 1px; }
        .rp-bottom .rpb-next { margin-right: 4px; }
        .rp-time {
            color: #c0bfbc;
            font-family: monospace, monospace;
            font-feature-settings: "tnum";
            min-width: 3.2em;
        }
        .rp-time-dim { color: #9a9996; }
        scale.rp-seek { margin: 0 2px; }
        scale.rp-vol { margin: 0; }
        scale.rp-vol > trough { min-height: 4px; }
        scale.rp-vol slider {
            min-width: 16px;
            min-height: 16px;
        }
        /* Smaller thumb than default libadwaita touch slab so trough clicks succeed for short seeks. */
        scale.rp-seek slider {
            min-width: 16px;
            min-height: 16px;
        }
        scale.rp-seek > trough {
            background-color: #3d3d3d;
            min-height: 6px;
            border-radius: 3px;
        }
        scale.rp-seek > trough > highlight {
            background-color: #78aeed;
            border-radius: 3px;
        }
        scale.rp-seek > trough > fill {
            background-color: #78aeed;
        }
        /* Header popovers: one shared layout for sound, subtitles, and speed. */
        popover.rp-header-popover > contents {
            padding: 0;
        }
        .rp-popover-box {
            padding: 12px;
            border-spacing: 10px;
            min-width: 240px;
        }
        .rp-popover-box scrolledwindow {
            min-height: 0;
        }
        .rp-popover-box list.rich-list {
            background: none;
        }
        /* Seek hover preview (see docs/features/18-thumbnail-preview.md) */
        frame.rp-seek-thumb-frame {
            padding: 3px;
            background-color: #2d2d2d;
            border: 1px solid rgba(255, 255, 255, 0.16);
            border-radius: 8px;
            box-shadow: 0 8px 22px rgba(0, 0, 0, 0.45);
        }
        frame.rp-seek-thumb-frame > border {
            border: none;
        }
        frame.rp-seek-thumb-frame glarea {
            border-radius: 5px;
        }
        label.rp-seek-thumb-chapter {
            color: #e0dfd8;
            font-size: 0.80em;
            padding: 0 4px;
        }
        label.rp-seek-thumb-time {
            color: #c0bfbc;
            font-size: 0.82em;
            font-family: monospace, monospace;
            font-feature-settings: "tnum";
            padding: 0 2px 1px 2px;
        }
    "#;

/// Continue strip: cards, Open tile, undo pill, progress styling ([`recent_view::fill_row`]).
const RECENT_GRID_CSS: &str = include_str!("theme_continue_grid.css");

/// macOS hybrid render: window + video stack transparent so the native mpv layer shows
/// through the GLArea. Continue grid and toolbar chrome keep their own opaque backgrounds.
const MACOS_TRANSPARENT_CONTENT_CSS: &str = r#"
        window.rp-win,
        .rp-stack,
        .rp-page-stack,
        .rp-gl.rp-gl-native {
            background: transparent;
            background-color: transparent;
        }
        .rp-recent-scroll,
        .rp-recent-vbox {
            background-color: #242424;
            background: #242424;
        }
        toolbarview.rp-toolbar headerbar.rpb-header {
            background-color: #2d2d2d;
            background: #2d2d2d;
        }
        toolbarview.rp-toolbar box.rp-bottom-shell {
            background-color: #2d2d2d;
            background: #2d2d2d;
            border-top: 1px solid #3d3d3d;
        }
        toolbarview.rp-toolbar box.rp-bottom-shell box.rp-bottom {
            background-color: transparent;
            background: transparent;
        }
    "#;

pub fn apply() {
    let mut css = String::with_capacity(
        APP_CSS.len()
            + RECENT_GRID_CSS.len()
            + 256
            + if cfg!(target_os = "macos") {
                MACOS_TRANSPARENT_CONTENT_CSS.len()
            } else {
                0
            }
            + 8,
    );
    css.push_str(APP_CSS);
    css.push_str(RECENT_GRID_CSS);
    if cfg!(target_os = "macos") {
        css.push_str(MACOS_TRANSPARENT_CONTENT_CSS);
    }
    crate::theme_cursor::append_cursor_css(&mut css);
    let p = gtk::CssProvider::new();
    p.load_from_string(&css);
    if let Some(d) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &d,
            &p,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    // GTK's default title-bar double-click toggles maximize after our HeaderBar gesture runs,
    // undoing `maximize()` before `fullscreen()` (first double-click looks maximized then snaps small).
    // Enter / menu use `toggle_fullscreen` only; disabling GDK's built-in action keeps parity.
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_titlebar_double_click(Some("none"));
        // GtkScale / GtkRange: primary click on trough jumps the thumb under the pointer and
        // keeps dragging until release (macOS Homebrew GTK often defaults this to false).
        settings.set_gtk_primary_button_warps_slider(true);
    }
}
