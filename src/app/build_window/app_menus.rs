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
    menu_append_action_icon(
        m,
        Some(SMOOTH60_MENU_LABEL),
        Some("app.smooth-60"),
        Some("camera-video-symbolic"),
    );
    menu_append_action_icon(
        m,
        Some(SEEK_BAR_MENU_LABEL),
        Some("app.seek-bar-preview"),
        Some("sidebar-show-symbolic"),
    );
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
    root.append_submenu(Some("File"), &build_macos_file_menu());
    root.append_submenu(Some("View"), &build_macos_view_menu(pref_menu));
    root
}

/// One labelled action row with an icon ([menu_append_action_icon] with `Some` wrappers).
#[cfg(target_os = "macos")]
fn macos_menu_item(menu: &gio::Menu, label: &str, action: &str, icon: &str) {
    menu_append_action_icon(menu, Some(label), Some(action), Some(icon));
}

/// File menu: open/close section plus exit-after-current and move-to-trash.
#[cfg(target_os = "macos")]
fn build_macos_file_menu() -> gio::Menu {
    let file = gio::Menu::new();
    let file_open_close = gio::Menu::new();
    macos_menu_item(
        &file_open_close,
        "Open Video…",
        "app.open",
        "document-open-symbolic",
    );
    macos_menu_item(
        &file_open_close,
        "Close Video",
        "app.close-video",
        "window-close-symbolic",
    );
    file.append_section(None::<&str>, &file_open_close);

    let file_extra = gio::Menu::new();
    macos_menu_item(
        &file_extra,
        "Exit After Current Video",
        "app.exit-after-current",
        "object-select-symbolic",
    );
    macos_menu_item(
        &file_extra,
        "Move to Trash",
        "app.move-to-trash",
        "user-trash-symbolic",
    );
    file.append_section(None::<&str>, &file_extra);
    file
}

/// View menu: enter-full-screen row plus the shared Preferences subtree.
#[cfg(target_os = "macos")]
fn build_macos_view_menu(pref_menu: &gio::Menu) -> gio::Menu {
    let view = gio::Menu::new();
    macos_menu_item(
        &view,
        "Enter Full Screen",
        "app.toggle-fullscreen",
        "view-fullscreen-symbolic",
    );
    view.append_submenu(Some("Preferences"), pref_menu);
    view
}
