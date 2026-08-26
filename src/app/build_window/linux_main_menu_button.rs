/// GNOME/Linux primary menu: [`gtk::MenuButton`] + [`gio::Menu`] so GTK builds a standard
/// [`gtk::PopoverMenu`] (libadwaita styling). Item text only at the top level — no custom icon rows.
#[cfg(not(target_os = "macos"))]
fn build_linux_main_menu_button(pref_menu: &gio::Menu) -> gtk::MenuButton {
    let mb = gtk::MenuButton::new();
    mb.set_icon_name("open-menu-symbolic");
    mb.set_tooltip_text(Some("Main menu"));
    mb.set_menu_model(Some(&build_main_menu_model(pref_menu)));
    wire_menu_button_popover_non_modal(&mb);
    mb
}

#[cfg(not(target_os = "macos"))]
fn build_main_menu_model(pref_menu: &gio::Menu) -> gio::Menu {
    let menu = gio::Menu::new();
    append_action_section(
        &menu,
        &[
            ("Open Video…", "app.open"),
            ("Close Video", "app.close-video"),
        ],
    );
    append_action_section(
        &menu,
        &[
            ("Exit After Current Video", "app.exit-after-current"),
            ("Move to Trash", "app.move-to-trash"),
        ],
    );
    append_action_section(&menu, &[("Fullscreen", "app.toggle-fullscreen")]);
    menu.append_submenu(Some("Preferences"), pref_menu);
    append_action_section(
        &menu,
        &[("About Rhino Player", "app.about"), ("Quit", "app.quit")],
    );
    menu
}

/// One labeled section whose items all trigger actions without icons.
#[cfg(not(target_os = "macos"))]
fn append_action_section(menu: &gio::Menu, items: &[(&str, &str)]) {
    let sec = gio::Menu::new();
    for (label, action) in items {
        menu_append_action_icon(&sec, Some(*label), Some(*action), None);
    }
    menu.append_section(None::<&str>, &sec);
}

/// Keep the popover non-modal however it is opened (click, keyboard, or initial setup).
#[cfg(not(target_os = "macos"))]
fn wire_menu_button_popover_non_modal(mb: &gtk::MenuButton) {
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
}
