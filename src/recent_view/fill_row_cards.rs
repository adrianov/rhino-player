/// Opens the same file-picker flow as **`app.open`** (native dialog on Linux / macOS).
fn open_video_pick_card() -> gtk::Overlay {
    let card = gtk::Overlay::new();
    card.set_vexpand(false);
    card.set_hexpand(false);
    card.set_overflow(gtk::Overflow::Hidden);
    card.add_css_class("rp-recent-card");
    card.add_css_class("rp-recent-open-pick");
    card.set_tooltip_text(Some(
        "Choose a video file — same action as Open Video from the menu",
    ));

    let btn = gtk::Button::builder()
        .action_name("app.open")
        .vexpand(false)
        .hexpand(true)
        .css_classes(["flat"])
        .build();
    btn.set_can_shrink(true);

    let col = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .vexpand(true)
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Center)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let im = gtk::Image::from_icon_name("list-add-symbolic");
    im.set_pixel_size(44);
    im.set_icon_size(gtk::IconSize::Large);
    im.set_valign(gtk::Align::Center);
    im.set_halign(gtk::Align::Center);
    im.add_css_class("rp-recent-open-pick-plus");

    let lab = gtk::Label::builder()
        .label("Open Video…")
        .single_line_mode(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .justify(gtk::Justification::Center)
        .build();
    lab.add_css_class("rp-recent-open-pick-label");

    col.append(&im);
    col.append(&lab);
    btn.set_child(Some(&col));
    card.set_child(Some(&btn));

    card.set_cursor_from_name(Some("pointer"));
    btn.set_cursor_from_name(Some("pointer"));
    card
}

/// Replace all children with cards. [on_remove] is **Remove from list**; [on_trash] is **Move to Trash**.
pub fn fill_row(
    row: &gtk::Box,
    items: Vec<CardData>,
    on_open: Rc<dyn Fn(&Path)>,
    on_remove: Rc<dyn Fn(&Path)>,
    on_trash: Rc<dyn Fn(&Path)>,
    warm_hover: Option<&WarmHoverHooks>,
    chrome_cache: Option<&crate::media_probe::ContinueGridCache>,
) {
    if let Some(cache) = chrome_cache {
        crate::media_probe::continue_grid_cache_refresh(cache, &items);
    }
    clear(row);
    let cards = Rc::new(RefCell::new(Vec::<gtk::Overlay>::new()));

    let pick = open_video_pick_card();
    let wrap_pick = adw::Clamp::new();
    wrap_pick.set_maximum_size(CARD_MAX_W);
    wrap_pick.set_child(Some(&pick));
    let (dw, dh) = default_card_dims();
    apply_card_dims(&pick, dw, dh);
    cards.borrow_mut().push(pick.clone());
    row.append(&wrap_pick);

    let handlers = HistoryCardHandlers {
        on_open,
        on_remove,
        on_trash,
        warm_hover,
    };
    for d in items {
        append_history_card(row, &cards, d, &handlers);
    }
    sync_card_sizes(row, &cards.borrow());
    let cards2 = Rc::clone(&cards);
    let hrow = row.clone();
    if let Some(parent) = hrow.parent() {
        let h = hrow.clone();
        let c = Rc::clone(&cards2);
        parent.connect_notify_local(Some("width"), move |_, _| {
            sync_card_sizes(&h, &c.borrow());
        });
    } else {
        let c = Rc::clone(&cards2);
        row.connect_notify_local(Some("width"), move |r, _| {
            sync_card_sizes(r, &c.borrow());
        });
    }
    // After the first [Allocation], parent width is reliable; [idle] runs after a layout pass in
    // case [notify] did not run when width crossed 0 → >0.
    let hrow = row.clone();
    let c3 = Rc::clone(&cards2);
    let _ = glib::idle_add_local(move || {
        sync_card_sizes(&hrow, &c3.borrow());
        glib::ControlFlow::Break
    });
}
