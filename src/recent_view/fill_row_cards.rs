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
    let btn = open_pick_button();
    card.set_child(Some(&btn));
    card.set_cursor_from_name(Some("pointer"));
    btn.set_cursor_from_name(Some("pointer"));
    card
}

fn open_pick_button() -> gtk::Button {
    let btn = gtk::Button::builder()
        .action_name("app.open")
        .vexpand(false)
        .hexpand(true)
        .css_classes(["flat"])
        .build();
    btn.set_can_shrink(true);
    btn.set_child(Some(&open_pick_content()));
    btn
}

fn open_pick_content() -> gtk::Box {
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
    col.append(&open_pick_plus_icon());
    col.append(&open_pick_caption());
    col
}

fn open_pick_plus_icon() -> gtk::Image {
    let im = gtk::Image::from_icon_name("list-add-symbolic");
    im.set_pixel_size(44);
    im.set_icon_size(gtk::IconSize::Large);
    im.set_valign(gtk::Align::Center);
    im.set_halign(gtk::Align::Center);
    im.add_css_class("rp-recent-open-pick-plus");
    im
}

fn open_pick_caption() -> gtk::Label {
    let lab = gtk::Label::builder()
        .label("Open Video…")
        .single_line_mode(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .justify(gtk::Justification::Center)
        .build();
    lab.add_css_class("rp-recent-open-pick-label");
    lab
}

/// Keep the existing Open Video tile when present; otherwise rebuild it.
fn keep_open_pick(row: &gtk::Box, cards: &Rc<RefCell<Vec<gtk::Overlay>>>) {
    if let Some(pick) = leading_open_pick(row) {
        cards.borrow_mut().push(pick);
        return;
    }
    clear(row);
    append_open_pick_tile(row, cards);
}

fn leading_open_pick(row: &gtk::Box) -> Option<gtk::Overlay> {
    let first = row.first_child()?;
    let clamp = first.downcast::<adw::Clamp>().ok()?;
    let child = clamp.child()?;
    let ov = child.downcast::<gtk::Overlay>().ok()?;
    ov.has_css_class("rp-recent-open-pick").then_some(ov)
}

/// Remove every child after the Open Video tile.
fn drop_after_first(row: &gtk::Box) {
    while row
        .last_child()
        .is_some_and(|last| row.first_child().as_ref() != Some(&last))
    {
        if let Some(last) = row.last_child() {
            last.unparent();
        }
    }
}

fn append_open_pick_tile(row: &gtk::Box, cards: &Rc<RefCell<Vec<gtk::Overlay>>>) {
    let pick = open_video_pick_card();
    let wrap_pick = adw::Clamp::new();
    wrap_pick.set_maximum_size(CARD_MAX_W);
    wrap_pick.set_child(Some(&pick));
    let (dw, dh) = default_card_dims();
    apply_card_dims(&pick, dw, dh);
    cards.borrow_mut().push(pick.clone());
    row.append(&wrap_pick);
}

/// Card action wiring shared by every card of one strip paint ([fill_row]).
pub struct StripActions {
    pub on_open: Rc<dyn Fn(&Path)>,
    pub on_remove: Rc<dyn Fn(&Path)>,
    pub on_trash: Rc<dyn Fn(&Path)>,
    pub warm_hover: Option<WarmHoverHooks>,
}

/// Replace trailing history cards; keeps the leading Open Video tile when present.
pub fn fill_row(
    row: &gtk::Box,
    items: Vec<CardData>,
    actions: StripActions,
    chrome_cache: Option<&crate::media_probe::ContinueGridCache>,
    kind: StripKind,
    cards: &Rc<RefCell<Vec<gtk::Overlay>>>,
    size_wired: &std::cell::Cell<bool>,
) {
    if let Some(cache) = chrome_cache.filter(|_| kind == StripKind::ContinueList) {
        crate::media_probe::continue_grid_cache_refresh(cache, &items);
    }
    cards.borrow_mut().clear();
    keep_open_pick(row, cards);
    drop_after_first(row);

    let handlers = HistoryCardHandlers {
        on_open: actions.on_open,
        on_remove: actions.on_remove,
        on_trash: actions.on_trash,
        warm_hover: actions.warm_hover.as_ref(),
        kind,
    };
    for d in items {
        append_history_card(row, cards, d, &handlers);
    }
    if size_wired.get() {
        sync_card_sizes(row, &cards.borrow());
    } else {
        wire_card_size_sync(row, cards);
        size_wired.set(true);
    }
}
