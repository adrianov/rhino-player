/// GNOME/Linux primary menu: [`gtk::MenuButton`] + [`gio::Menu`] so GTK builds a standard
/// [`gtk::PopoverMenu`] (libadwaita styling). Item text only at the top level — no custom icon rows.
#[cfg(not(target_os = "macos"))]
fn build_linux_main_menu_button(pref_menu: &gio::Menu) -> gtk::MenuButton {
    let mb = gtk::MenuButton::new();
    mb.set_icon_name("open-menu-symbolic");
    mb.set_tooltip_text(Some("Main menu"));

    let menu = gio::Menu::new();

    let sec_file = gio::Menu::new();
    menu_append_action_icon(&sec_file, Some("Open Video…"), Some("app.open"), None);
    menu_append_action_icon(&sec_file, Some("Close Video"), Some("app.close-video"), None);
    menu.append_section(None::<&str>, &sec_file);

    let sec_session = gio::Menu::new();
    menu_append_action_icon(
        &sec_session,
        Some("Exit After Current Video"),
        Some("app.exit-after-current"),
        None,
    );
    menu_append_action_icon(&sec_session, Some("Move to Trash"), Some("app.move-to-trash"), None);
    menu.append_section(None::<&str>, &sec_session);

    let sec_view = gio::Menu::new();
    menu_append_action_icon(&sec_view, Some("Fullscreen"), Some("app.toggle-fullscreen"), None);
    menu.append_section(None::<&str>, &sec_view);

    menu.append_submenu(Some("Preferences"), pref_menu);

    let sec_about = gio::Menu::new();
    menu_append_action_icon(&sec_about, Some("About Rhino Player"), Some("app.about"), None);
    menu_append_action_icon(&sec_about, Some("Quit"), Some("app.quit"), None);
    menu.append_section(None::<&str>, &sec_about);

    mb.set_menu_model(Some(&menu));

    mb.connect_notify_local(Some("popover"), move |b, _| {
        if let Some(p) = b.popover() {
            header_popover_non_modal(&p);
        }
    });
    mb.connect_active_notify(move |b| {
        if b.is_active() {
            if let Some(p) = b.popover() {
                header_popover_non_modal(&p);
            }
        }
    });
    if let Some(p) = mb.popover() {
        header_popover_non_modal(&p);
    }
    mb
}
