use adw::prelude::*;
use gtk::prelude::WidgetExt;

pub(crate) const CARD_ASPECT: f64 = 16.0 / 9.0;
pub(crate) const CARD_MIN_W: i32 = 220;
pub(crate) const CARD_MAX_W: i32 = 620;
pub(crate) const CARD_GAP: i32 = 16;

/// 16:9 footprint used before the scrolled strip has a width (startup / first paint).
pub(crate) fn default_card_dims() -> (i32, i32) {
    let w = CARD_MIN_W;
    let h = (f64::from(w) / CARD_ASPECT).round() as i32;
    (w, h)
}

pub(crate) fn apply_card_dims(card: &gtk::Overlay, w: i32, h: i32) {
    card.set_size_request(w, h);
    if let Some(pw) = card.parent() {
        if let Some(clamp) = pw.downcast_ref::<adw::Clamp>() {
            clamp.set_maximum_size(w);
            clamp.set_size_request(w, h);
        }
    }
}

pub(crate) fn card_width(strip_w: i32, count: usize) -> i32 {
    let count = count.max(1) as i32;
    let avail = (strip_w - CARD_GAP * (count - 1)).max(CARD_MIN_W);
    let target = if count == 1 {
        (f64::from(strip_w) * 0.40).round() as i32
    } else {
        avail / count
    };
    target.clamp(CARD_MIN_W, CARD_MAX_W)
}

/// Width for [`card_width`] / [`sync_card_sizes`]: nearest ancestor
/// [`gtk::ScrolledWindow`], else window width, else a strip-wide fallback.
pub(crate) fn strip_width_for_cards(card_row: &gtk::Box) -> i32 {
    let mut w_opt = card_row.parent();
    while let Some(w) = w_opt {
        if let Some(sw) = w.downcast_ref::<gtk::ScrolledWindow>() {
            let ww = sw.width();
            if ww > 0 {
                return ww;
            }
        }
        w_opt = w.parent();
    }
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

pub(crate) fn sync_card_sizes(card_row: &gtk::Box, cards: &[gtk::Overlay]) {
    if cards.is_empty() {
        return;
    }
    let strip_w = strip_width_for_cards(card_row);
    let w = card_width(strip_w, cards.len());
    let h = (f64::from(w) / CARD_ASPECT).round() as i32;
    for card in cards {
        apply_card_dims(card, w, h);
    }
}
