use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::glib::prelude::Cast;
use gtk::prelude::EventControllerExt;
use gtk::prelude::IsA;

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

include!("strip_stack.rs");
include!("live_card.rs");
include!("undo_bar_scroll_new_row/recent_context.rs");
