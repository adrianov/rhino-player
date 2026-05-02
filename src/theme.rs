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
        }
        label.rp-fs-clock {
            color: #c0bfbc;
            font-family: monospace, monospace;
            font-feature-settings: "tnum";
            margin-right: 10px;
            font-size: 0.95em;
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
        .rp-stack {
            background-color: #242424;
        }
        .rp-gl { background: #000000; min-height: 120px; }
        /* macOS native render: GLArea publishes alpha=0 pixels (cleared in
           connect_render) so the underlying CAOpenGLLayer (mpv video) shows through. */
        .rp-gl.rp-gl-native { background: transparent; background-color: transparent; }
        .rp-bottom {
            background-color: #1e1e1e;
            border-top: 1px solid #3d3d3d;
            padding: 8px 14px 12px 14px;
        }
        /* LTR clock spacing; avoid margin-inline-end (not in all GTK CSS parsers). */
        .rp-bottom .rpb-prev { margin-right: 2px; }
        .rp-bottom .rpb-play { margin-left: 2px; margin-right: 2px; }
        .rp-bottom .rpb-next { margin-right: 6px; }
        .rp-time {
            color: #c0bfbc;
            font-family: monospace, monospace;
            font-feature-settings: "tnum";
            min-width: 3.2em;
        }
        .rp-time-dim { color: #9a9996; }
        scale.rp-seek { margin: 0 4px; }
        scale.rp-vol { margin: 0; }
        scale.rp-vol > trough { min-height: 4px; }
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
        popover.menu.rp-main-menu-popover contents {
            padding: 6px;
        }
        .rp-main-menu-box {
            min-width: 240px;
        }
        button.rp-main-menu-act.flat {
            border-radius: 6px;
            padding: 4px 8px;
            min-height: 40px;
        }
        button.rp-main-menu-act.flat:hover {
            background-color: rgba(255, 255, 255, 0.07);
        }
        .rp-main-menu-act checkbutton {
            margin: 0;
        }
        menubutton.rp-main-menu-act.flat:hover {
            background-color: rgba(255, 255, 255, 0.07);
            border-radius: 6px;
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

/// macOS: when the GLArea (and its container chain) is transparent, the native video
/// CAOpenGLLayer **below** gdk's GTK sublayer shows through. The chrome (header / bottom
/// bar) keeps its own opaque backgrounds (`.rpb-header`, `.rp-bottom`) so they still
/// overlay the video; the recent grid (`.rp-recent-scroll`) keeps its dark base so it
/// covers the (now-transparent) video region when shown.
const MACOS_TRANSPARENT_CONTENT_CSS: &str = r#"
        window.rp-win,
        .rp-stack,
        .rp-page-stack,
        .rp-gl.rp-gl-native {
            background: transparent;
            background-color: transparent;
        }
    "#;

/// GTK ≥ 4.12 adds `cursor` to the CSS dialect on typical Linux builds. Some stacks (macOS / Homebrew
/// in particular) still report a new `gtk::minor_version()` yet reject `cursor` in
/// [CssProvider::load_from_string] with “No property named cursor”, which breaks parsing of our sheet.
const CURSOR_CSS: &str = r#"
        button:not(:disabled), menubutton:not(:disabled), modelbutton, togglebutton:not(:disabled),
        switch, checkbutton, radiobutton, link, scale:not(:disabled), spinbutton > button,
        listview > row, listbox > row {
            cursor: pointer;
        }
        button:disabled, menubutton:disabled, scale:disabled, togglebutton:disabled {
            cursor: not-allowed;
        }
        glarea.rp-cursor-hidden {
            cursor: none;
        }
    "#;

fn append_cursor_css(css: &mut String) {
    if cfg!(target_os = "macos") {
        return;
    }
    if gtk::minor_version() >= 12 {
        css.push_str(CURSOR_CSS);
    }
}

pub fn apply() {
    let mut css = String::with_capacity(
        APP_CSS.len()
            + RECENT_GRID_CSS.len()
            + CURSOR_CSS.len()
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
    append_cursor_css(&mut css);
    let p = gtk::CssProvider::new();
    p.load_from_string(&css);
    if let Some(d) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &d,
            &p,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
