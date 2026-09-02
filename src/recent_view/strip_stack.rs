// Continue-strip layout assembly: the vertical band (spacer / search row / card scroller /
// undo + notice pills / spacer) and the horizontal card row with its scroller. The undo pill
// widget itself stays in `undo_bar_scroll_new_row.rs`.

/// Everything [new_scroll] builds for the continue screen, owned by [WindowWidgets].
pub struct ScrollArea {
    /// Vertical band: search row, spacers, card scroller, undo + notice pills.
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

    ScrollArea {
        recent_scrl: v,
        flow_recent: h,
        spacers,
        undo_bar,
        notice_toast,
        search,
    }
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

/// Vertical stack: top spacer (search centered in its free space), card scroller, undo pill
/// band ([new_undo_bar]), notice band ([new_notice_toast]), bottom spacer. The two
/// `[gtk::Box]` spacers are the **empty** hit area for main-window double-click fullscreen.
///
/// Returns the mounted undo/notice handles so callers wire visibility to the on-screen pills.
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
    let sp_bot = bottom_spacer();
    let undo_bar = new_undo_bar();
    let notice = new_notice_toast();
    v.append(&sp_top);
    v.append(card_scr);
    v.append(&undo_bar.shell);
    v.append(&notice.shell);
    v.append(&sp_bot);

    (v, [sp_top, sp_bot], undo_bar, notice)
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

fn bottom_spacer() -> gtk::Box {
    let sp = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sp.set_vexpand(true);
    sp
}

fn flex_filler() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
    b.set_vexpand(true);
    b
}
