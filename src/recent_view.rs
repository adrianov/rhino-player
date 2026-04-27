//! Recent video cards for empty launch. See [docs/features/21-recent-videos-launch.md].

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

fn card_width(row_w: i32, count: usize) -> i32 {
    let count = count.max(1) as i32;
    let avail = (row_w - CARD_GAP * (count - 1)).max(CARD_MIN_W);
    let target = if count == 1 {
        (f64::from(row_w) * 0.40).round() as i32
    } else {
        avail / count
    };
    target.clamp(CARD_MIN_W, CARD_MAX_W)
}

fn sync_card_sizes(row: &gtk::Box, cards: &[gtk::Overlay]) {
    if cards.is_empty() {
        return;
    }
    let w = card_width(row.width(), cards.len());
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

/// Creates or reuses a [RecentContext] in [cell] (one per window).
pub fn ensure_recent_backfill(
    cell: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    row: &gtk::Box,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
) -> Rc<RecentContext> {
    if let Some(c) = cell.borrow().as_ref() {
        return Rc::clone(c);
    }
    let cancel = Arc::new(AtomicBool::new(true));
    let (refill_tx, refill_rx) = mpsc::channel();
    let ctx = Rc::new(RecentContext {
        row: row.clone(),
        on_open,
        on_remove,
        on_trash,
        cancel: cancel.clone(),
        refill_tx,
        poll_id: Rc::new(RefCell::new(None)),
        workers: Rc::new(RefCell::new(Vec::new())),
    });
    let c_poll = Rc::clone(&ctx);
    // [Receiver] is main-thread only; the timer callback runs on the GTK main thread.
    let rxm = Rc::new(RefCell::new(refill_rx));
    let c_rx = Rc::clone(&rxm);
    let id = glib::source::timeout_add_local(Duration::from_millis(32), move || {
        let mut n = 0u32;
        {
            let g = c_rx.borrow_mut();
            while g.try_recv().is_ok() {
                n += 1;
            }
        }
        if n > 0 {
            c_poll.refill();
        }
        glib::ControlFlow::Continue
    });
    *ctx.poll_id.borrow_mut() = Some(id);
    *cell.borrow_mut() = Some(Rc::clone(&ctx));
    ctx
}

/// For each path, if the file is present and the DB has no up-to-date thumb, runs [media_probe::ensure_thumbnail] on a **worker** thread, then [RecentContext::refill] on the main loop via a [Send] channel.
/// Safe to call from the main thread: does not block on libmpv.
pub fn schedule_thumb_backfill(ctx: Rc<RecentContext>, paths: Vec<std::path::PathBuf>) {
    let tx = ctx.refill_tx.clone();
    let c = ctx.cancel.clone();
    let h = std::thread::spawn(move || {
        for p in paths {
            if !c.load(Ordering::Acquire) {
                return;
            }
            if !p.exists() {
                continue;
            }
            let can = match std::fs::canonicalize(&p) {
                Ok(c) => c,
                _ => continue,
            };
            if media_probe::cached_thumbnail_for_path(&can).is_some() {
                continue;
            }
            let _ = media_probe::ensure_thumbnail(&can);
            if !c.load(Ordering::Acquire) {
                return;
            }
            if tx.send(()).is_err() {
                return;
            }
        }
    });
    ctx.workers.borrow_mut().push(h);
}

/// Hand on hover, primary click triggers [act]. [show_on_hover] (e.g. trash + remove) is shown on hover.
/// Uses [PropagationPhase::Target] so nested [gtk::Button]s receive the click first.
fn add_click_and_pointer(
    card: &impl IsA<gtk::Widget>,
    debug_path: &str,
    act: UnitFn,
    show_on_hover: &[gtk::Button],
) {
    card.as_ref().set_can_target(true);
    let g = gtk::GestureClick::new();
    g.set_button(1);
    g.set_propagation_phase(gtk::PropagationPhase::Target);
    let act = act.clone();
    let p = debug_path.to_string();
    g.connect_pressed(move |_, n, _x, _y| {
        eprintln!("[rhino] recent: gesture pressed n={n} path={p}");
        if n == 1 {
            eprintln!("[rhino] recent: invoking open/remove handler");
            act(());
        } else {
            eprintln!("[rhino] recent: ignored n!=1 (if stuck, n may be wrong for this GTK/WM)");
        }
    });
    card.as_ref().add_controller(g);

    let c = card.as_ref().clone();
    let show: Vec<gtk::Button> = show_on_hover.to_vec();
    let m = gtk::EventControllerMotion::new();
    m.connect_enter(move |_, _x, _y| {
        c.set_cursor_from_name(Some("pointer"));
        for b in &show {
            b.set_visible(true);
        }
    });
    let c = card.as_ref().clone();
    let hide: Vec<gtk::Button> = show_on_hover.to_vec();
    m.connect_leave(move |_| {
        c.set_cursor_from_name(None);
        for b in &hide {
            b.set_visible(false);
        }
    });
    card.as_ref().add_controller(m);
}

/// Replace all children with cards. [on_remove] drops an entry from the list; [on_trash] moves a file
/// to the system Trash then removes it from the list.
pub fn fill_row(
    row: &gtk::Box,
    items: Vec<CardData>,
    on_open: Rc<dyn Fn(&Path)>,
    on_remove: Rc<dyn Fn(&Path)>,
    on_trash: Rc<dyn Fn(&Path)>,
) {
    clear(row);
    let cards = Rc::new(RefCell::new(Vec::<gtk::Overlay>::new()));
    for d in items {
        let c = d.path.clone();
        let miss = d.missing;
        let p = d.percent;
        let name = c
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let a11y = format!("{name}, {p:.0} percent played");

        let card = gtk::Overlay::new();
        card.set_vexpand(false);
        card.set_hexpand(false);
        card.set_overflow(gtk::Overflow::Hidden);
        card.add_css_class("rp-recent-card");
        if miss {
            card.add_css_class("rp-stale");
        }
        let tip = if miss {
            format!(
                "{}\n{} — file missing, click to remove from recent",
                c.display(),
                a11y
            )
        } else {
            format!("{}\n{a11y}", c.display(), a11y = a11y)
        };
        card.set_tooltip_text(Some(tip.as_str()));

        let bg: gtk::Widget = if miss {
            full_bleed_icon("image-missing-symbolic")
        } else if let Some(ref bytes) = d.thumb {
            if let Some(tex) = crate::jpeg_texture::texture_from_jpeg(bytes.as_slice()) {
                let pic = gtk::Picture::for_paintable(&tex);
                pic.set_content_fit(gtk::ContentFit::Cover);
                pic.set_can_shrink(true);
                pic.set_vexpand(true);
                pic.set_hexpand(true);
                pic.set_halign(gtk::Align::Fill);
                pic.set_valign(gtk::Align::Fill);
                no_target(&pic);
                pic.add_css_class("rp-recent-bg");
                pic.upcast()
            } else {
                full_bleed_icon("video-x-generic")
            }
        } else {
            full_bleed_icon("video-x-generic")
        };
        card.set_child(Some(&bg));

        let footer = gtk::Box::new(gtk::Orientation::Vertical, 6);
        footer.set_halign(gtk::Align::Fill);
        footer.set_valign(gtk::Align::End);
        no_target(&footer);
        footer.add_css_class("rp-recent-card-footer");

        let label = gtk::Label::new(Some(&name));
        no_target(&label);
        label.add_css_class("rp-recent-card-title");
        label.set_ellipsize(gtk::pango::EllipsizeMode::None);
        label.set_max_width_chars(-1);
        label.set_wrap(true);
        label.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
        label.set_tooltip_text(c.to_str());
        label.set_halign(gtk::Align::Fill);
        label.set_xalign(0.0);

        let pro = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        no_target(&pro);
        pro.add_css_class("rp-recent-progress-row");
        let bar = gtk::ProgressBar::new();
        no_target(&bar);
        bar.set_fraction(p / 100.0);
        bar.set_show_text(false);
        bar.set_hexpand(true);
        bar.add_css_class("rp-recent-bar");
        let lp = gtk::Label::new(Some(&format!("{p:.0}%")));
        no_target(&lp);
        lp.add_css_class("rp-recent-percent");
        pro.append(&bar);
        pro.append(&lp);

        footer.append(&label);
        footer.append(&pro);
        card.add_overlay(&footer);

        let top_actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        top_actions.set_spacing(2);
        top_actions.set_halign(gtk::Align::End);
        top_actions.set_valign(gtk::Align::Start);
        top_actions.set_margin_top(2);
        top_actions.set_margin_end(2);

        let dismiss = gtk::Button::from_icon_name("window-close-symbolic");
        dismiss.set_visible(false);
        dismiss.set_tooltip_text(Some("Remove from list"));
        dismiss.add_css_class("flat");
        dismiss.add_css_class("circular");
        dismiss.add_css_class("rp-recent-dismiss");
        {
            let path = c.clone();
            let rem = on_remove.clone();
            dismiss.connect_clicked(move |_| {
                eprintln!("[rhino] recent: dismiss path={}", path.display());
                rem(&path);
            });
        }

        let hover_btns: Vec<gtk::Button> = if !miss && c.is_file() {
            let trash = gtk::Button::from_icon_name("user-trash-symbolic");
            trash.set_visible(false);
            trash.set_tooltip_text(Some("Move to trash"));
            trash.add_css_class("flat");
            trash.add_css_class("circular");
            trash.add_css_class("rp-recent-trash");
            {
                let path = c.clone();
                let tr = on_trash.clone();
                trash.connect_clicked(move |_| {
                    eprintln!("[rhino] recent: trash path={}", path.display());
                    tr(&path);
                });
            }
            top_actions.append(&trash);
            top_actions.append(&dismiss);
            vec![trash, dismiss]
        } else {
            top_actions.append(&dismiss);
            vec![dismiss.clone()]
        };
        card.add_overlay(&top_actions);

        if miss {
            let path = c.clone();
            let rem = on_remove.clone();
            add_click_and_pointer(
                &card,
                &c.display().to_string(),
                Rc::new(move |()| {
                    eprintln!(
                        "[rhino] recent: stale remove callback path={}",
                        path.display()
                    );
                    rem(&path);
                }),
                &hover_btns,
            );
        } else {
            let path = c.clone();
            let op = on_open.clone();
            add_click_and_pointer(
                &card,
                &c.display().to_string(),
                Rc::new(move |()| {
                    eprintln!("[rhino] recent: open callback path={}", path.display());
                    op(&path);
                }),
                &hover_btns,
            );
        }

        let wrap = adw::Clamp::new();
        wrap.set_maximum_size(CARD_MAX_W);
        wrap.set_child(Some(&card));
        cards.borrow_mut().push(card.clone());
        row.append(&wrap);
    }
    sync_card_sizes(row, &cards.borrow());
    let cards2 = Rc::clone(&cards);
    row.connect_notify_local(Some("width"), move |r, _| {
        sync_card_sizes(r, &cards2.borrow());
    });
}

/// Probes each path in an idle; [card_data_list] is DB-only (no libmpv) on the main thread, then
/// [schedule_backfill] starts missing-cache work when the owner decides it will not compete with startup.
pub fn fill_idle(
    row: &gtk::Box,
    paths: Vec<std::path::PathBuf>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    schedule_backfill: BackfillFn,
) {
    let row = row.clone();
    let o = on_open;
    let r = on_remove;
    let t = on_trash;
    let _ = glib::idle_add_local(move || {
        eprintln!(
            "[rhino] recent: fill_idle build grid for {} path(s):",
            paths.len()
        );
        for p in &paths {
            eprintln!("[rhino] recent:   candidate {}", p.display());
        }
        let n = ensure_recent_backfill(&backfill, &row, o.clone(), r.clone(), t.clone());
        let v: Vec<CardData> = card_data_list(&paths);
        eprintln!("[rhino] recent: card_data done ({} cards)", v.len());
        for cd in &v {
            eprintln!(
                "[rhino] recent:   card path={} missing={}",
                cd.path.display(),
                cd.missing
            );
        }
        fill_row(&row, v, o.clone(), r.clone(), t.clone());
        let paths_t = paths.clone();
        schedule_backfill(n, paths_t);
        glib::ControlFlow::Break
    });
}
