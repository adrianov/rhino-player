
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
    shell.set_hexpand(true);
    shell.set_halign(gtk::Align::Fill);
    shell.set_valign(gtk::Align::Start);
    shell.set_vexpand(false);
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

/// Scrolled row of at most five continue cards, with the undo snackbar **under** the strip but
/// outside the horizontal scroller — the pill stays centered on the viewport when the strip scrolls.
///
/// The two `[gtk::Box]` spacers (top, bottom) are the **empty** hit area for main-window
/// double-click fullscreen: not the card strip or undo bar.
pub fn new_scroll() -> (gtk::Box, gtk::Box, [gtk::Box; 2], UndoBar) {
    let h = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    // Equal width comes from [sync_card_sizes]; homogeneous only stretches height to the
    // tallest child's natural size (thumbnails / Open tile) before the first sync on Linux.
    h.set_homogeneous(false);
    h.set_halign(gtk::Align::Center);
    h.set_baseline_position(gtk::BaselinePosition::Top);
    h.set_vexpand(false);
    h.set_hexpand(false);
    h.add_css_class("rp-recent-row");

    let card_scr = gtk::ScrolledWindow::builder()
        .child(&h)
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
    v.append(&card_scr);
    v.append(&undo_bar.shell);
    v.append(&sp_bot);

    (v, h, [sp_top, sp_bot], undo_bar)
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

type UnitFn = Rc<dyn Fn(()) + 'static>;
type RcPathFn = Rc<dyn Fn(&Path) + 'static>;
type BackfillFn = Rc<dyn Fn(Rc<RecentContext>, Vec<std::path::PathBuf>) + 'static>;
type WarmHoverLeave = Rc<dyn Fn()>;

/// Debounced warm-preload hooks for continue-card pointer enter/leave.
#[derive(Clone)]
pub struct WarmHoverHooks {
    pub enter: RcPathFn,
    pub leave: WarmHoverLeave,
}

/// Per-window state for the recent row: [refill] after background thumbs, [shutdown] on scroll destroy.
pub struct RecentContext {
    chrome_cache: crate::media_probe::ContinueGridCache,
    /// Same box as the grid row; used by [refill].
    row: gtk::Box,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    warm_hover: Option<WarmHoverHooks>,
    /// Stops workers and poller; cleared in [shutdown].
    pub cancel: Arc<AtomicBool>,
    /// Worker → main: request a [refill] (no GTK types on the [Send] side).
    refill_tx: mpsc::Sender<()>,
    /// Main-loop timer that drains [refill_tx] and calls [refill] on this context.
    poll_id: Rc<RefCell<Option<glib::SourceId>>>,
    /// Background thumb threads (joined in [shutdown]).
    workers: Rc<RefCell<Vec<JoinHandle<()>>>>,
    /// Incremented on each [schedule_thumb_backfill]; stale workers exit between files.
    backfill_gen: Arc<std::sync::atomic::AtomicU64>,
}

impl RecentContext {
    pub(crate) fn warm_hover(&self) -> Option<&WarmHoverHooks> {
        self.warm_hover.as_ref()
    }

    /// Rebuilds cards from the current history (first five paths).
    pub fn refill(&self) {
        let paths: Vec<std::path::PathBuf> = crate::history::load()
            .into_iter()
            .take(CONTINUE_DISPLAY_MAX)
            .collect();
        let v: Vec<CardData> = card_data_list(&paths);
        fill_row(
            &self.row,
            v,
            self.on_open.clone(),
            self.on_remove.clone(),
            self.on_trash.clone(),
            self.warm_hover.as_ref(),
            Some(&self.chrome_cache),
        );
    }

    /// Stops the poller, signals workers to exit, and **detaches** worker joins to a short-lived
    /// background thread (does **not** block the GTK main thread: [media_probe::ensure_thumbnail] can
    /// run many seconds; cancel is checked only between files, not inside libmpv).
    pub fn shutdown(&self) {
        self.cancel.store(false, Ordering::Release);
        crate::glib_source_drop::drop_glib_source(self.poll_id.as_ref());
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

