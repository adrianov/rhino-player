
use adw::prelude::*;
use gtk::glib;
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

const CARD_ASPECT: f64 = 5.0 / 3.0;
const CARD_MIN_W: i32 = 220;
const CARD_MAX_W: i32 = 620;
const CARD_GAP: i32 = 16;

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
    let label = gtk::Label::new(None);
    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    label.set_max_width_chars(44);
    label.set_xalign(0.0);
    label.set_halign(gtk::Align::Start);
    label.set_valign(gtk::Align::Center);
    label.set_single_line_mode(true);
    label.set_hexpand(true);
    label.add_css_class("rp-undo-toast-text");

    let undo = gtk::Button::with_label("Undo");
    undo.set_tooltip_text(Some(
        "Put the most recently removed file back on the continue list",
    ));
    undo.set_valign(gtk::Align::Center);
    undo.set_halign(gtk::Align::Center);
    undo.add_css_class("flat");
    undo.add_css_class("rp-undo-toast-undo");
    undo.set_cursor_from_name(Some("pointer"));

    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.set_valign(gtk::Align::Center);
    close.set_halign(gtk::Align::Center);
    close.set_tooltip_text(Some("Dismiss"));
    close.add_css_class("circular");
    close.add_css_class("flat");
    close.add_css_class("rp-undo-toast-close");
    close.set_cursor_from_name(Some("pointer"));

    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bar.set_spacing(6);
    bar.set_halign(gtk::Align::Center);
    bar.set_valign(gtk::Align::Center);
    bar.append(&label);
    bar.append(&undo);
    bar.append(&close);
    bar.add_css_class("rp-undo-toast");

    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shell.set_halign(gtk::Align::Center);
    shell.set_valign(gtk::Align::Start);
    shell.set_vexpand(false);
    shell.set_hexpand(false);
    shell.set_visible(false);
    shell.set_margin_top(4);
    shell.set_margin_start(16);
    shell.set_margin_end(16);
    shell.add_css_class("rp-undo-shell");
    shell.append(&bar);

    UndoBar {
        shell,
        label,
        undo,
        close,
    }
}

/// Scrolled row of at most five continue cards, with the undo snackbar **under** the strip (in-layout, not under the window bottom toolbar).
///
/// The four `[gtk::Box]` spacers (top, left, right, bottom around the **card** row) are the **empty** hit
/// area for main-window double-click fullscreen: not the card strip or undo bar.
pub fn new_scroll() -> (gtk::ScrolledWindow, gtk::Box, [gtk::Box; 4], UndoBar) {
    let h = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    h.set_halign(gtk::Align::Center);
    h.set_baseline_position(gtk::BaselinePosition::Top);
    h.set_vexpand(false);
    h.set_hexpand(false);
    h.add_css_class("rp-recent-row");

    let sp_left = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sp_left.set_hexpand(true);
    sp_left.set_vexpand(true);
    let sp_right = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sp_right.set_hexpand(true);
    sp_right.set_vexpand(true);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_halign(gtk::Align::Fill);
    row.set_valign(gtk::Align::Start);
    row.set_hexpand(true);
    row.set_vexpand(false);
    row.append(&sp_left);
    row.append(&h);
    row.append(&sp_right);

    let v = gtk::Box::new(gtk::Orientation::Vertical, 0);
    v.set_vexpand(true);
    v.set_hexpand(true);
    v.set_halign(gtk::Align::Fill);
    v.set_valign(gtk::Align::Fill);
    v.add_css_class("rp-recent-vbox");

    let sp_top = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sp_top.set_vexpand(true);
    let sp_bot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sp_bot.set_vexpand(true);
    let undo_bar = new_undo_bar();
    v.append(&sp_top);
    v.append(&row);
    v.append(&undo_bar.shell);
    v.append(&sp_bot);

    let s = gtk::ScrolledWindow::new();
    s.set_child(Some(&v));
    s.set_vexpand(true);
    s.set_hexpand(true);
    s.set_halign(gtk::Align::Fill);
    s.set_vscrollbar_policy(gtk::PolicyType::Never);
    s.set_hscrollbar_policy(gtk::PolicyType::Automatic);
    s.set_kinetic_scrolling(false);
    s.add_css_class("rp-recent-scroll");
    (s, h, [sp_top, sp_left, sp_right, sp_bot], undo_bar)
}

fn clear(f: &gtk::Box) {
    while let Some(c) = f.first_child() {
        c.unparent();
    }
}

fn card_width(strip_w: i32, count: usize) -> i32 {
    let count = count.max(1) as i32;
    let avail = (strip_w - CARD_GAP * (count - 1)).max(CARD_MIN_W);
    let target = if count == 1 {
        (f64::from(strip_w) * 0.40).round() as i32
    } else {
        avail / count
    };
    target.clamp(CARD_MIN_W, CARD_MAX_W)
}

/// The card row [gtk::Box] (`rp-recent-row`) sits inside a full-width parent; use that width for
/// the 40% / multi-card math. Measuring the inner box couples card size to its own `size_request`,
/// so a hover relayout (e.g. overlay actions) can change the number and make the card jump.
fn strip_width_for_cards(card_row: &gtk::Box) -> i32 {
    card_row
        .parent()
        .map(|p| p.width())
        .filter(|&w| w > 0)
        .unwrap_or_else(|| card_row.width().max(1))
}

fn sync_card_sizes(card_row: &gtk::Box, cards: &[gtk::Overlay]) {
    if cards.is_empty() {
        return;
    }
    let w = card_width(strip_width_for_cards(card_row), cards.len());
    let h = (f64::from(w) / CARD_ASPECT).round() as i32;
    for card in cards {
        card.set_size_request(w, h);
        card.set_width_request(w);
        card.set_height_request(h);
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

type UnitFn = Rc<dyn Fn(()) + 'static>;
type RcPathFn = Rc<dyn Fn(&Path) + 'static>;
type BackfillFn = Rc<dyn Fn(Rc<RecentContext>, Vec<std::path::PathBuf>) + 'static>;

/// Per-window state for the recent row: [refill] after background thumbs, [shutdown] on scroll destroy.
pub struct RecentContext {
    /// Same box as the grid row; used by [refill].
    row: gtk::Box,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    /// Stops workers and poller; cleared in [shutdown].
    pub cancel: Arc<AtomicBool>,
    /// Worker → main: request a [refill] (no GTK types on the [Send] side).
    refill_tx: mpsc::Sender<()>,
    /// Main-loop timer that drains [refill_tx] and calls [refill] on this context.
    poll_id: Rc<RefCell<Option<glib::SourceId>>>,
    /// Background thumb threads (joined in [shutdown]).
    workers: Rc<RefCell<Vec<JoinHandle<()>>>>,
}

impl RecentContext {
    /// Rebuilds cards from the current history (first five paths).
    pub fn refill(&self) {
        let paths: Vec<std::path::PathBuf> = crate::history::load().into_iter().take(5).collect();
        let v: Vec<CardData> = card_data_list(&paths);
        fill_row(
            &self.row,
            v,
            self.on_open.clone(),
            self.on_remove.clone(),
            self.on_trash.clone(),
        );
    }

    /// Stops the poller, signals workers to exit, and **detaches** worker joins to a short-lived
    /// background thread (does **not** block the GTK main thread: [media_probe::ensure_thumbnail] can
    /// run many seconds; cancel is checked only between files, not inside libmpv).
    pub fn shutdown(&self) {
        self.cancel.store(false, Ordering::Release);
        if let Some(id) = self.poll_id.borrow_mut().take() {
            id.remove();
        }
        let workers: Vec<JoinHandle<()>> = self.workers.borrow_mut().drain(..).collect();
        if workers.is_empty() {
            return;
        }
        if let Err(e) = std::thread::Builder::new()
            .name("rhino-recent-join".to_string())
            .spawn(move || {
                for h in workers {
                    let _ = h.join();
                }
            })
        {
            eprintln!("[rhino] recent: joiner spawn: {e}");
        }
    }
}

