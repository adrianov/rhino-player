use adw::prelude::*;
use gtk::prelude::WidgetExt;

/// Same footprint as grid thumb decode ([crate::thumb_texture::GRID_CARD_ASPECT]).
pub(crate) const CARD_ASPECT: f64 = crate::thumb_texture::GRID_CARD_ASPECT;
/// Continue strip shows at most this many history cards (plus Open Video).
pub(crate) const CONTINUE_DISPLAY_MAX: usize = 5;
pub(crate) const CARD_MIN_W: i32 = 220;
pub(crate) const CARD_MAX_W: i32 = 620;
pub(crate) const CARD_GAP: i32 = 16;

/// 16:9 footprint used before the scrolled strip has a width (startup / first paint).
pub(crate) fn default_card_dims() -> (i32, i32) {
    let w = CARD_MIN_W;
    (w, card_height(w))
}

pub(crate) fn apply_card_dims(card: &gtk::Overlay, w: i32, h: i32) {
    card.set_size_request(w, h);
    // Start — not Fill — so a tall natural child cannot stretch siblings when the row grows.
    card.set_valign(gtk::Align::Start);
    card.set_halign(gtk::Align::Center);
    if let Some(pw) = card.parent() {
        if let Some(clamp) = pw.downcast_ref::<adw::Clamp>() {
            clamp.set_maximum_size(w);
            clamp.set_size_request(w, h);
            clamp.set_valign(gtk::Align::Start);
        }
    }
}

/// Landscape height for a card width ([CARD_ASPECT]).
pub(crate) fn card_height(w: i32) -> i32 {
    (f64::from(w) / CARD_ASPECT).round() as i32
}

/// Tile width for the strip. Always divides by a full strip (Open + [CONTINUE_DISPLAY_MAX])
/// so a short list or search hit does not inflate cards and shove the layout around.
pub(crate) fn card_width(strip_w: i32) -> i32 {
    let slots = (CONTINUE_DISPLAY_MAX + 1) as i32;
    let avail = (strip_w - CARD_GAP * (slots - 1)).max(CARD_MIN_W);
    (avail / slots).clamp(CARD_MIN_W, CARD_MAX_W)
}

fn ancestor_scrolled_width(card_row: &gtk::Box) -> Option<i32> {
    let mut w_opt = card_row.parent();
    while let Some(w) = w_opt {
        if let Some(sw) = w.downcast_ref::<gtk::ScrolledWindow>() {
            let ww = sw.width();
            if ww > 0 {
                return Some(ww);
            }
        }
        w_opt = w.parent();
    }
    None
}

fn strip_fallback_width(card_row: &gtk::Box) -> i32 {
    if let Some(fb) = card_row.parent() {
        let fbw = fb.width();
        if fbw > 0 {
            return fbw;
        }
    }
    if let Some(win) = card_row
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok())
    {
        let ww = win.width().max(win.default_width());
        if ww > 0 {
            return ww;
        }
    }
    CARD_MIN_W * 3 + CARD_GAP * 2
}

/// Width for [`card_width`] / [`sync_card_sizes`]: nearest ancestor
/// [`gtk::ScrolledWindow`], else window width, else a strip-wide fallback.
pub(crate) fn strip_width_for_cards(card_row: &gtk::Box) -> i32 {
    if let Some(w) = ancestor_scrolled_width(card_row) {
        return w;
    }
    strip_fallback_width(card_row)
}

pub(crate) fn sync_card_sizes(card_row: &gtk::Box, cards: &[gtk::Overlay]) {
    if cards.is_empty() {
        return;
    }
    let strip_w = strip_width_for_cards(card_row);
    let w = card_width(strip_w);
    let h = card_height(w);
    for card in cards {
        apply_card_dims(card, w, h);
    }
}

pub(crate) fn wire_card_size_sync(row: &gtk::Box, cards: &Rc<RefCell<Vec<gtk::Overlay>>>) {
    wire_width_notify(row, cards);
    schedule_first_size_sync(row, cards);
}

fn wire_width_notify(row: &gtk::Box, cards: &Rc<RefCell<Vec<gtk::Overlay>>>) {
    let hrow = row.clone();
    if let Some(parent) = hrow.parent() {
        let h = hrow.clone();
        let c = Rc::clone(cards);
        parent.connect_notify_local(Some("width"), move |_, _| {
            sync_card_sizes(&h, &c.borrow());
        });
    } else {
        let c = Rc::clone(cards);
        row.connect_notify_local(Some("width"), move |r, _| {
            sync_card_sizes(r, &c.borrow());
        });
    }
}

fn schedule_first_size_sync(row: &gtk::Box, cards: &Rc<RefCell<Vec<gtk::Overlay>>>) {
    let hrow = row.clone();
    let c3 = Rc::clone(cards);
    let _ = glib::idle_add_local(move || {
        sync_card_sizes(&hrow, &c3.borrow());
        glib::ControlFlow::Break
    });
}

#[cfg(test)]
mod card_width_tests {
    use super::*;

    #[test]
    fn short_strip_matches_full_strip_width() {
        let strip = 1400;
        let full = card_width(strip);
        assert!((CARD_MIN_W..=CARD_MAX_W).contains(&full));
        // Same formula regardless of how many cards are showing (caller no longer passes count).
        assert_eq!(full, (strip - CARD_GAP * CONTINUE_DISPLAY_MAX as i32) / (CONTINUE_DISPLAY_MAX as i32 + 1));
    }

    #[test]
    fn card_height_stays_landscape() {
        for w in [CARD_MIN_W, 400, CARD_MAX_W] {
            let h = card_height(w);
            assert!(h < w, "expected 16:9 height {h} < width {w}");
            assert_eq!(h, (f64::from(w) / CARD_ASPECT).round() as i32);
        }
    }
}
