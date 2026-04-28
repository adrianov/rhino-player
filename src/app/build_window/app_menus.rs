/// Builds the application's GMenu models: the main hamburger menu and the
/// nested **Preferences** submenu.
///
/// Action ids are bound by the application (`app.open`, `app.smooth-60`, etc.);
/// this helper only builds the static structure consumed by `MenuButton`.
fn build_app_menus() -> (gio::Menu, gio::Menu) {
    let pref_menu = gio::Menu::new();
    pref_menu.append(Some(SMOOTH60_MENU_LABEL), Some("app.smooth-60"));
    pref_menu.append(
        Some("Choose VapourSynth Script (.vpy)…"),
        Some("app.choose-vs"),
    );
    let menu = gio::Menu::new();
    menu.append(Some("Open Video…"), Some("app.open"));
    menu.append(Some("Close Video"), Some("app.close-video"));
    menu.append(
        Some("Exit After Current Video"),
        Some("app.exit-after-current"),
    );
    menu.append(Some("Move to Trash"), Some("app.move-to-trash"));
    menu.append_submenu(Some("Preferences"), &pref_menu);
    menu.append(Some("About Rhino Player"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));
    (menu, pref_menu)
}
