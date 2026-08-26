use gtk::glib;
use gtk::glib::prelude::Cast;
use gtk::prelude::EventControllerExt;
use gtk::prelude::IsA;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::media_probe::{self, card_data_list, CardData};

/// Session undo: title, **Undo**, close (dismisses without restoring). Placed in [new_scroll] under the card row.
/// Plain [gtk::Box] shell (not [gtk::Revealer]) so GTK does not paint an extra background plane behind the pill.
pub struct UndoBar {
    /// Wraps the pill; visibility toggles; must stay visually transparent.
    pub shell: gtk::Box,
    pub label: gtk::Label,
    pub undo: gtk::Button,
    pub close: gtk::Button,
}

/// Pill-style bar; inserted in the continue [gtk::Box] directly below the thumbnail row.
fn new_undo_bar() -> UndoBar {
    let label = undo_label();
    let undo = undo_button();
    let close = dismiss_button();

    let bar = undo_pill_bar(&label, &undo, &close);
    let shell = toast_shell(&bar, &[]);

    UndoBar {
        shell,
        label,
        undo,
        close,
    }
}

/// Horizontal pill row: title, **Undo**, dismiss.
fn undo_pill_bar(label: &gtk::Label, undo: &gtk::Button, close: &gtk::Button) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bar.set_spacing(6);
    bar.set_halign(gtk::Align::Center);
    bar.set_valign(gtk::Align::Center);
    bar.append(label);
    bar.append(undo);
    bar.append(close);
    bar.add_css_class("rp-undo-toast");
    bar
}

fn undo_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    label.set_max_width_chars(44);
    label.set_xalign(0.0);
    label.set_halign(gtk::Align::Start);
    label.set_valign(gtk::Align::Center);
    label.set_single_line_mode(true);
    label.set_hexpand(true);
    label.add_css_class("rp-undo-toast-text");
    label
}

fn undo_button() -> gtk::Button {
    let undo = gtk::Button::with_label("Undo");
    undo.set_tooltip_text(Some("Restore the last remove or trash"));
    undo.set_valign(gtk::Align::Center);
    undo.set_halign(gtk::Align::Center);
    undo.add_css_class("flat");
    undo.add_css_class("rp-undo-toast-undo");
    undo.set_cursor_from_name(Some("pointer"));
    undo
}

/// Scrolled row of at most five continue cards, with the undo snackbar **under** the strip but
/// outside the horizontal scroller — the pill stays centered on the viewport when the strip scrolls.
/// Open-failure notices share that under-strip band ([NoticeToast]).
///
/// The two `[gtk::Box]` spacers (top, bottom) are the **empty** hit area for main-window
/// double-click fullscreen: not the card strip or undo bar.
pub fn new_scroll() -> (gtk::Box, gtk::Box, [gtk::Box; 2], UndoBar, NoticeToast) {
    let h = recent_strip_row();
    let card_scr = recent_card_scroller(&h);
    let (v, spacers) = recent_stack(&card_scr);

    (v, h, spacers, new_undo_bar(), new_notice_toast())
}

/// Horizontal row that hosts the continue cards.
fn recent_strip_row() -> gtk::Box {
    let h = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    // Equal width comes from [sync_card_sizes]; homogeneous only stretches height to the
    // tallest child's natural size (thumbnails / Open tile) before the first sync on Linux.
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

/// Vertical stack: top spacer, scroller, undo pill band ([new_undo_bar]), notice band
/// ([new_notice_toast]), bottom spacer. The two `[gtk::Box]` spacers are the **empty** hit
/// area for main-window double-click fullscreen: not the card strip or undo bar.
fn recent_stack(card_scr: &gtk::ScrolledWindow) -> (gtk::Box, [gtk::Box; 2]) {
    let v = gtk::Box::new(gtk::Orientation::Vertical, 0);
    v.set_vexpand(true);
    v.set_hexpand(true);
    v.set_halign(gtk::Align::Fill);
    v.set_valign(gtk::Align::Fill);
    v.add_css_class("rp-recent-vbox");

    let [sp_top, sp_bot] = stack_spacers();
    let undo_bar = new_undo_bar();
    let notice = new_notice_toast();
    v.append(&sp_top);
    v.append(card_scr);
    v.append(&undo_bar.shell);
    v.append(&notice.shell);
    v.append(&sp_bot);

    (v, [sp_top, sp_bot])
}

/// Top / bottom flexible spacers flanking the strip.
fn stack_spacers() -> [gtk::Box; 2] {
    let sp_top = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sp_top.set_vexpand(true);
    let sp_bot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sp_bot.set_vexpand(true);
    [sp_top, sp_bot]
}

fn clear(f: &gtk::Box) {
    while let Some(c) = f.first_child() {
        c.unparent();
    }
}

fn no_target(w: &impl IsA<gtk::Widget>) {
    w.set_can_target(false);
}

/// Centered icon on a full-card panel (stale or no thumbnail).
fn full_bleed_icon(icon: &'static str) -> gtk::Widget {
    let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
    bx.set_vexpand(true);
    bx.set_hexpand(true);
    bx.set_halign(gtk::Align::Fill);
    bx.set_valign(gtk::Align::Fill);
    bx.add_css_class("rp-recent-bg-miss");
    let im = gtk::Image::from_icon_name(icon);
    im.set_vexpand(false);
    im.set_valign(gtk::Align::Center);
    im.set_halign(gtk::Align::Center);
    im.set_icon_size(gtk::IconSize::Large);
    im.add_css_class("rp-recent-pict");
    no_target(&im);
    bx.append(&im);
    no_target(&bx);
    bx.upcast()
}

include!("undo_bar_scroll_new_row/recent_context.rs");
