/// Builds the application's [gio::Menu] models: the **Preferences** subtree and, on macOS,
/// the hierarchical menubar root. Linux uses the same [gio::Menu] with [gtk::MenuButton::set_menu_model]
/// for the primary menu (`linux_main_menu_button.rs`); placeholder empty menus fill tuple slots there.
///
/// Action ids match [gio::SimpleAction]s on the application; this helper only
/// builds static structure consumed by [gtk::MenuButton] or `Application::set_menubar`.
fn build_app_menus() -> (gio::Menu, gio::Menu, gio::Menu) {
    let pref_menu = gio::Menu::new();
    menu_pref_append_smooth_and_seek_skeleton(&pref_menu);
    #[cfg(not(target_os = "macos"))]
    {
        (gio::Menu::new(), pref_menu, gio::Menu::new())
    }
    #[cfg(target_os = "macos")]
    {
        let menubar = build_macos_menubar(&pref_menu);
        (gio::Menu::new(), pref_menu, menubar)
    }
}

/// Initial rows before [video_pref_submenu_rebuild]; it calls [gio::Menu::remove_all].
fn menu_pref_append_smooth_and_seek_skeleton(m: &gio::Menu) {
    menu_append_action_icon(m, Some(SMOOTH60_MENU_LABEL), Some("app.smooth-60"), Some("camera-video-symbolic"));
    menu_append_action_icon(m, Some(SEEK_BAR_MENU_LABEL), Some("app.seek-bar-preview"), Some("sidebar-show-symbolic"));
    menu_append_action_icon(
        m,
        Some("Choose VapourSynth Script (.vpy)…"),
        Some("app.choose-vs"),
        Some("document-properties-symbolic"),
    );
}

#[cfg(target_os = "macos")]
fn build_macos_menubar(pref_menu: &gio::Menu) -> gio::Menu {
    let root = gio::Menu::new();

    let file = gio::Menu::new();
    let file_open_close = gio::Menu::new();
    menu_append_action_icon(&file_open_close, Some("Open Video…"), Some("app.open"), Some("document-open-symbolic"));
    menu_append_action_icon(&file_open_close, Some("Close Video"), Some("app.close-video"), Some("window-close-symbolic"));
    file.append_section(None::<&str>, &file_open_close);

    let file_extra = gio::Menu::new();
    menu_append_action_icon(
        &file_extra,
        Some("Exit After Current Video"),
        Some("app.exit-after-current"),
        Some("object-select-symbolic"),
    );
    menu_append_action_icon(&file_extra, Some("Move to Trash"), Some("app.move-to-trash"), Some("user-trash-symbolic"));
    file.append_section(None::<&str>, &file_extra);

    root.append_submenu(Some("File"), &file);

    let view = gio::Menu::new();
    menu_append_action_icon(
        &view,
        Some("Enter Full Screen"),
        Some("app.toggle-fullscreen"),
        Some("view-fullscreen-symbolic"),
    );
    view.append_submenu(Some("Preferences"), pref_menu);
    root.append_submenu(Some("View"), &view);

    root
}
