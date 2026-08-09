/// Pointer / not-allowed cursors on controls (GTK CSS `cursor` when the runtime parser supports it).
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

pub fn append_cursor_css(css: &mut String) {
    if cfg!(target_os = "macos") {
        return;
    }
    if gtk_supports_cursor_css() {
        css.push_str(CURSOR_CSS);
    }
}

/// Distro GTK 4.14.x still rejects `cursor` in CSS ("No property named cursor"); 4.16+ is the
/// first runtime gate we use. Custom hit targets use [`gtk::WidgetExt::set_cursor_from_name`].
fn gtk_supports_cursor_css() -> bool {
    gtk::check_version(4, 16, 0).is_none()
}
