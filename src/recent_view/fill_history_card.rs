#[path = "fill_history_card/card_actions.rs"]
mod card_actions;
use card_actions::top_action_buttons;

/// Whole-card click: Remove on stale cards, Open otherwise.
fn attach_card_activation(
    card: &gtk::Overlay,
    c: &Path,
    h: &HistoryCardHandlers<'_>,
    miss: bool,
    card_warm: Option<&WarmHoverHooks>,
    hover_btns: &[gtk::Button],
) {
    if miss {
        let path = c.to_path_buf();
        let rem = h.on_remove.clone();
        add_click_and_pointer(
            card,
            c,
            Rc::new(move |()| {
                crate::user_action_log::act(format!("continue remove {}", path.display()));
                rem(&path);
            }),
            hover_btns,
            card_warm,
        );
    } else {
        let path = c.to_path_buf();
        let op = h.on_open.clone();
        add_click_and_pointer(
            card,
            c,
            Rc::new(move |()| {
                crate::user_action_log::act(format!("continue open {}", path.display()));
                op(&path);
            }),
            hover_btns,
            card_warm,
        );
    }
}

/// Title / accessibility strings and progress for a history card.
fn history_card_texts(d: &CardData) -> (std::path::PathBuf, String, String) {
    let name = d
        .path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let label_txt = crate::human_media_title::human_media_title(&name);
    let a11y = format!("{label_txt}, {:.0} percent played", d.percent);
    (d.path.clone(), label_txt, a11y)
}

/// Card chrome: overlay, stale styling, tooltip, full-bleed background.
fn fresh_history_card(d: &CardData, miss: bool, tip: &str) -> gtk::Overlay {
    let card = gtk::Overlay::new();
    card.set_vexpand(false);
    card.set_hexpand(false);
    card.set_overflow(gtk::Overflow::Hidden);
    card.add_css_class("rp-recent-card");
    if miss {
        card.add_css_class("rp-stale");
    }
    card.set_tooltip_text(Some(tip));
    card.set_child(Some(&card_background(d, miss)));
    card
}

/// Bottom overlay: wrapped title above the progress row.
fn history_footer(label_txt: &str, c: &Path, p: f64) -> gtk::Box {
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 6);
    footer.set_halign(gtk::Align::Fill);
    footer.set_valign(gtk::Align::End);
    footer.set_hexpand(true);
    no_target(&footer);
    footer.add_css_class("rp-recent-card-footer");
    footer.append(&card_title_label(label_txt, c));
    footer.append(&progress_row(p));
    footer
}

/// Default dims, clamp wrapper, registry push, and insertion into the strip.
fn finish_history_card(
    row: &gtk::Box,
    cards: &Rc<RefCell<Vec<gtk::Overlay>>>,
    card: &gtk::Overlay,
) {
    let (w, h) = default_card_dims();
    apply_card_dims(card, w, h);

    let wrap = adw::Clamp::new();
    wrap.set_maximum_size(CARD_MAX_W);
    wrap.set_child(Some(card));
    cards.borrow_mut().push(card.clone());
    row.append(&wrap);
}

fn append_history_card(
    row: &gtk::Box,
    cards: &Rc<RefCell<Vec<gtk::Overlay>>>,
    d: CardData,
    h: &HistoryCardHandlers<'_>,
) {
    let (c, label_txt, a11y) = history_card_texts(&d);
    let p = d.percent;
    let miss = d.missing;
    let tip = history_card_tooltip(&c, &a11y, miss);
    let card = fresh_history_card(&d, miss, &tip);
    card.add_overlay(&history_footer(&label_txt, &c, p));

    let (top_actions, hover_btns) = top_action_buttons(&c, h, miss);
    if !hover_btns.is_empty() {
        card.add_overlay(&top_actions);
    }
    let card_warm = if miss { None } else { h.warm_hover };
    attach_card_activation(&card, &c, h, miss, card_warm, &hover_btns);
    finish_history_card(row, cards, &card);
}

include!("fill_history_card/history_card_widgets.rs");
