// Continue-strip layout: vertical band (search spacer / card scroller / bottom overlay
// spacer) and the horizontal card row. Undo / notice overlay that bottom spacer so they
// do not reflow the strip. The undo pill widget stays in `undo_bar_scroll_new_row.rs`.

/// Everything [new_scroll] builds for the continue screen, owned by [WindowWidgets].
pub struct ScrollArea {
    /// Vertical band: search row, spacers, card scroller; undo / notice overlay the bottom.
    pub recent_scrl: gtk::Box,
    /// Horizontal row hosting the cards.
    pub flow_recent: gtk::Box,
    /// Empty double-click-fullscreen hit areas around the strip.
    pub spacers: [gtk::Box; 2],
    pub undo_bar: UndoBar,
    pub notice_toast: NoticeToast,
    /// Neighbour-search box + I'm Feeling Lucky mounted atop the strip (feature 33).
    pub search: SiblingSearch,
}

pub fn new_scroll() -> ScrollArea {
    let h = recent_strip_row();
    let card_scr = recent_card_scroller(&h);
    let search = SiblingSearch::new();
    // Must return the same undo/notice widgets that `recent_stack` mounts — a second
    // `new_undo_bar`/`new_notice_toast` here would wire actions to orphaned pills.
    let (v, spacers, undo_bar, notice_toast) = recent_stack(&card_scr, search.widget());
    wire_strip_scroll_everywhere(&v, &card_scr);

    ScrollArea {
        recent_scrl: v,
        flow_recent: h,
        spacers,
        undo_bar,
        notice_toast,
        search,
    }
}

/// Two-finger / wheel scroll anywhere on the continue screen pans the card strip through a
/// single pan path: a capture-phase controller on the whole band takes every scroll event
/// before the strip's own `ScrolledWindow` and drives the strip's h-adjustment directly, so
/// travel over the cards equals travel over the surrounding band by construction — two
/// handlers with two scalings (the scroller's native step increment vs the pan stride) would
/// disagree. The native scroller only receives what the pan refuses (hidden strip, edge rest,
/// collapsed range — states in which it cannot move either), so no scroll is ever applied
/// twice. Cards keep their own click and hover interactions; the scrollbar drag is a gesture,
/// not a scroll event, and stays native.
fn wire_strip_scroll_everywhere(v: &gtk::Box, card_scr: &gtk::ScrolledWindow) {
    use gtk::prelude::WidgetExt;

    let card = card_scr.clone();
    // No DISCRETE flag: it quantizes accumulated smooth (touchpad) deltas to integers, which
    // both makes touchpans chunky and defeats any delta-shape classification. Wheel events
    // still arrive; the scroll unit below separates them exactly.
    let sc = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    sc.set_propagation_phase(gtk::PropagationPhase::Capture);
    sc.connect_scroll(move |sc2, dx, dy| {
        if !card.is_visible()
            || !pan_strip(card.hadjustment(), strip_scroll_delta(sc2.unit(), dx, dy))
        {
            return glib::Propagation::Proceed;
        }
        glib::Propagation::Stop
    });
    v.add_controller(sc);
}

/// Fixed pan per wheel notch.
const STRIP_WHEEL_STEP: f64 = 48.0;

/// Scroll distance for a strip pan: wheel notches step a fixed amount, surface-unit events
/// (touchpad pans) track finger travel in pixels 1:1. Classification uses the event's scroll
/// unit — never delta fractionality: smooth deltas can arrive integral (and GTK's DISCRETE
/// flag would even quantize them to integers).
fn strip_scroll_delta(unit: gtk::gdk::ScrollUnit, dx: f64, dy: f64) -> f64 {
    let d = dx + dy;
    if unit == gtk::gdk::ScrollUnit::Wheel {
        d * STRIP_WHEEL_STEP
    } else {
        d
    }
}


/// Pans the card strip by `d` pixels; false when the strip cannot move (hidden, d is zero,
/// already at the edge, or the adjustment range is collapsed — e.g. content fits, or a layout
/// pass leaves `page_size` at the range size — where `upper - page_size` would undercut
/// `lower` and `f64::clamp` would panic) so the event keeps bubbling instead of being consumed.
fn pan_strip(adj: gtk::Adjustment, d: f64) -> bool {
    use gtk::prelude::AdjustmentExt;

    if d == 0.0 {
        return false;
    }
    let max = adj.upper() - adj.page_size();
    if max <= adj.lower() {
        return false;
    }
    let val = (adj.value() + d).clamp(adj.lower(), max);
    if (val - adj.value()).abs() < f64::EPSILON {
        return false;
    }
    adj.set_value(val);
    true
}

/// Horizontal row that hosts the continue cards.
fn recent_strip_row() -> gtk::Box {
    let h = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    // Equal width/height from [sync_card_sizes] (fixed 16:9). Stills are cover-cropped and
    // painted via [crate::thumb_texture] so a portrait WebP cannot raise the row.
    h.set_homogeneous(false);
    h.set_halign(gtk::Align::Center);
    h.set_baseline_position(gtk::BaselinePosition::Top);
    h.set_vexpand(false);
    h.set_hexpand(false);
    h.add_css_class("rp-recent-row");
    h
}

/// Horizontally scrollable wrapper for the strip row.
fn recent_card_scroller(h: &gtk::Box) -> gtk::ScrolledWindow {
    let card_scr = gtk::ScrolledWindow::builder()
        .child(h)
        .vexpand(false)
        .hexpand(true)
        .halign(gtk::Align::Fill)
        .valign(gtk::Align::Start)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .kinetic_scrolling(false)
        .propagate_natural_height(true)
        .build();
    card_scr.add_css_class("rp-recent-scroll");
    card_scr
}

/// Vertical stack: top spacer (search parked above the strip), card scroller, bottom overlay
/// spacer. Undo / notice sit on that overlay just under the strip. The two `[gtk::Box]`
/// spacers are the empty hit area for main-window double-click fullscreen.
fn recent_stack(
    card_scr: &gtk::ScrolledWindow,
    search_row: &gtk::Box,
) -> (gtk::Box, [gtk::Box; 2], UndoBar, NoticeToast) {
    let v = gtk::Box::new(gtk::Orientation::Vertical, 0);
    v.set_vexpand(true);
    v.set_hexpand(true);
    v.set_halign(gtk::Align::Fill);
    v.set_valign(gtk::Align::Fill);
    v.add_css_class("rp-recent-vbox");

    let sp_top = top_spacer_with_search(search_row);
    let undo_bar = new_undo_bar();
    let notice = new_notice_toast();
    let (sp_bot, toast_ovl) = bottom_spacer_overlay(&undo_bar.shell, &notice.shell);
    v.append(&sp_top);
    v.append(card_scr);
    v.append(&toast_ovl);

    (v, [sp_top, sp_bot], undo_bar, notice)
}

/// Bottom expand spacer with undo / notice overlaid at its top. Overlay expand follows
/// the main child (`gtk_overlay_compute_expand`), and overlay children are not measured,
/// so showing a pill does not change the vbox split.
fn bottom_spacer_overlay(undo_shell: &gtk::Box, notice_shell: &gtk::Box) -> (gtk::Box, gtk::Overlay) {
    let ovl = gtk::Overlay::new();
    let hit = flex_filler();
    ovl.set_child(Some(&hit));
    let band = gtk::Box::new(gtk::Orientation::Vertical, 0);
    band.set_halign(gtk::Align::Center);
    band.set_valign(gtk::Align::Start);
    band.append(undo_shell);
    band.append(notice_shell);
    ovl.add_overlay(&band);
    (hit, ovl)
}

/// Top expand spacer with the search row parked just above the card strip (stable when
/// strip height changes — centering between equal fillers used to slide it vertically).
fn top_spacer_with_search(search_row: &gtk::Box) -> gtk::Box {
    let sp = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sp.set_vexpand(true);
    sp.set_hexpand(true);
    let above = flex_filler();
    // Pass double-clicks through to [sp] (fullscreen hit target).
    above.set_can_target(false);
    search_row.set_halign(gtk::Align::Center);
    search_row.set_valign(gtk::Align::End);
    search_row.set_hexpand(false);
    search_row.set_margin_bottom(16);
    sp.append(&above);
    sp.append(search_row);
    sp
}

fn flex_filler() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
    b.set_vexpand(true);
    b
}
