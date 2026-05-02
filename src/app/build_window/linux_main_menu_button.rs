/// GNOME/Linux hamburger panel: GTK 4 hides leading icons beside labels in [`GtkPopoverMenu`]
/// backed by [`gio::Menu`]; build explicit rows (**icon**, **label**) and wire [`gio::Action`]s.
#[cfg(not(target_os = "macos"))]
fn build_linux_main_menu_button(
    app: &adw::Application,
    pref_menu: &gio::Menu,
    exit_after_current: &Rc<Cell<bool>>,
) -> gtk::MenuButton {
    let mb = gtk::MenuButton::new();
    mb.set_icon_name("open-menu-symbolic");
    mb.set_tooltip_text(Some("Main menu"));

    let pop = gtk::Popover::builder().autohide(true).focusable(true).build();
    pop.add_css_class("menu");
    pop.add_css_class("rp-main-menu-popover");

    let col = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .css_classes(["rp-main-menu-box"])
        .build();

    fn row_with_icon(icon_name: &str, text: &str) -> gtk::Box {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .margin_start(12)
            .margin_end(12)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        let im = gtk::Image::from_icon_name(icon_name);
        im.set_pixel_size(18);
        im.set_valign(gtk::Align::Center);
        let lab = gtk::Label::builder().label(text).hexpand(true).xalign(0f32).build();
        row.append(&im);
        row.append(&lab);
        row
    }

    fn wire_action(icon: &'static str, label: &'static str, action: &'static str) -> gtk::Button {
        let b = gtk::Button::builder()
            .hexpand(true)
            .css_classes(["flat", "rp-main-menu-act"])
            .build();
        b.set_child(Some(&row_with_icon(icon, label)));
        b.set_action_name(Some(action));
        b
    }

    col.append(&wire_action("document-open-symbolic", "Open Video…", "app.open"));
    col.append(&wire_action("window-close-symbolic", "Close Video", "app.close-video"));
    col.append(&wire_action("view-fullscreen-symbolic", "Fullscreen", "app.toggle-fullscreen"));

    let exit_syncing = Rc::new(Cell::new(false));
    let exit_wrap = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_start(10)
        .margin_end(12)
        .margin_top(4)
        .margin_bottom(4)
        .css_classes(["rp-main-menu-act"])
        .build();
    let exit_ic = gtk::Image::from_icon_name("object-select-symbolic");
    exit_ic.set_pixel_size(18);
    exit_ic.set_valign(gtk::Align::Center);
    let exit_chk = gtk::CheckButton::builder()
        .label("Exit After Current Video")
        .hexpand(true)
        .build();
    exit_chk.set_margin_start(0);
    exit_wrap.append(&exit_ic);
    exit_wrap.append(&exit_chk);
    col.append(&exit_wrap);

    let app_chk = app.clone();
    let ex_sync_flag = exit_syncing.clone();
    exit_chk.connect_toggled(move |c| {
        if ex_sync_flag.get() {
            return;
        }
        let want = c.is_active();
        app_chk.change_action_state("exit-after-current", &want.to_variant());
    });

    col.append(&wire_action("user-trash-symbolic", "Move to Trash", "app.move-to-trash"));

    let pref_pop =
        gtk::PopoverMenu::from_model_full(pref_menu, gtk::PopoverMenuFlags::NESTED);
    header_popover_non_modal(&pref_pop);
    let pref_btn =
        gtk::MenuButton::builder().hexpand(true).popover(&pref_pop).direction(gtk::ArrowType::Right).build();
    pref_btn.add_css_class("flat");
    pref_btn.add_css_class("rp-main-menu-act");
    pref_btn.set_child(Some(&row_with_icon(
        "preferences-system-symbolic",
        "Preferences",
    )));

    col.append(&pref_btn);
    col.append(&wire_action(
        "help-about-symbolic",
        "About Rhino Player",
        "app.about",
    ));
    col.append(&wire_action("application-exit-symbolic", "Quit", "app.quit"));

    let ex_sync_pop = exit_syncing;
    let ex_cell = exit_after_current.clone();
    let chk_snap = exit_chk.clone();
    pop.connect_show(move |_| {
        ex_sync_pop.set(true);
        chk_snap.set_active(ex_cell.get());
        ex_sync_pop.set(false);
    });

    pop.set_child(Some(&col));
    mb.set_popover(Some(&pop));

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
