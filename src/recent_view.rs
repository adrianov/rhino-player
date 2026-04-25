//! Recent video cards for empty launch. See [docs/features/21-recent-videos-launch.md].

use adw::prelude::*;
use gtk::gdk;
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

/// Scrolled row of at most five cards; [undo] is the "Undo" button next to the revealer label.
pub fn new_scroll() -> (gtk::ScrolledWindow, gtk::Box, gtk::Revealer, gtk::Button) {
    let h = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    h.set_halign(gtk::Align::Center);
    h.set_baseline_position(gtk::BaselinePosition::Top);
    h.add_css_class("rp-recent-row");

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
    let undo_b = gtk::Button::with_label("Undo");
    undo_b.set_tooltip_text(Some("Restore the last removed item to this list"));
    undo_b.add_css_class("suggested-action");
    undo_b.set_halign(gtk::Align::Center);
    let undo_r = gtk::Box::new(gtk::Orientation::Vertical, 0);
    undo_r.set_halign(gtk::Align::Center);
    undo_r.append(&undo_b);
    let undo_revealer = gtk::Revealer::new();
    undo_revealer.set_reveal_child(false);
    undo_revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
    undo_revealer.set_child(Some(&undo_r));
    undo_revealer.add_css_class("rp-recent-undo");
    v.append(&sp_top);
    v.append(&undo_revealer);
    v.append(&h);
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
    (s, h, undo_revealer, undo_b)
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
    im.set_vexpand(true);
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

/// Per-window state for the recent row: [refill] after background thumbs, [shutdown] on scroll destroy.
pub struct RecentContext {
    /// Same box as the grid row; used by [refill].
    row: gtk::Box,
    on_open: RcPathFn,
    on_remove: RcPathFn,
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
        let paths: Vec<std::path::PathBuf> =
            crate::history::load().into_iter().take(5).collect();
        let v: Vec<CardData> = card_data_list(&paths);
        fill_row(
            &self.row,
            v,
            self.on_open.clone(),
            self.on_remove.clone(),
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

/// Hand on hover, primary click triggers [act]. [dismiss] is shown on hover (top-right remove).
/// Uses [PropagationPhase::Target] so a nested [gtk::Button] (dismiss) receives the click first.
fn add_click_and_pointer(
    card: &impl IsA<gtk::Widget>,
    debug_path: &str,
    act: UnitFn,
    dismiss: &gtk::Button,
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
    let dis = dismiss.clone();
    let m = gtk::EventControllerMotion::new();
    m.connect_enter(move |_, _x, _y| {
        c.set_cursor_from_name(Some("pointer"));
        dis.set_visible(true);
    });
    let c = card.as_ref().clone();
    let dis2 = dismiss.clone();
    m.connect_leave(move |_| {
        c.set_cursor_from_name(None);
        dis2.set_visible(false);
    });
    card.as_ref().add_controller(m);
}

/// Replace all children with cards. [on_remove] is used to drop an entry (missing file or dismiss control).
pub fn fill_row(
    row: &gtk::Box,
    items: Vec<CardData>,
    on_open: Rc<dyn Fn(&Path)>,
    on_remove: Rc<dyn Fn(&Path)>,
) {
    clear(row);
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
        card.set_size_request(200, 120);
        card.set_width_request(200);
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
            let b = glib::Bytes::from(bytes.as_slice());
            if let Ok(tex) = gdk::Texture::from_bytes(&b) {
                let pic = gtk::Picture::for_paintable(&tex);
                pic.set_content_fit(gtk::ContentFit::Cover);
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

        let footer = gtk::Box::new(gtk::Orientation::Vertical, 4);
        footer.set_halign(gtk::Align::Fill);
        footer.set_valign(gtk::Align::End);
        no_target(&footer);
        footer.add_css_class("rp-recent-card-footer");
        footer.set_margin_start(8);
        footer.set_margin_end(8);
        footer.set_margin_bottom(6);
        footer.set_margin_top(6);

        let label = gtk::Label::new(Some(&name));
        no_target(&label);
        label.set_ellipsize(gtk::pango::EllipsizeMode::None);
        label.set_max_width_chars(-1);
        label.set_wrap(true);
        label.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
        label.set_tooltip_text(c.to_str());
        label.set_halign(gtk::Align::Fill);
        label.set_xalign(0.0);

        let pro = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        no_target(&pro);
        let bar = gtk::ProgressBar::new();
        no_target(&bar);
        bar.set_fraction(p / 100.0);
        bar.set_show_text(false);
        bar.set_hexpand(true);
        bar.add_css_class("rp-recent-bar");
        let lp = gtk::Label::new(Some(&format!("{p:.0}%")));
        no_target(&lp);
        lp.add_css_class("dim-label");
        pro.append(&bar);
        pro.append(&lp);

        footer.append(&label);
        footer.append(&pro);
        card.add_overlay(&footer);

        let dismiss = gtk::Button::from_icon_name("window-close-symbolic");
        dismiss.set_valign(gtk::Align::Start);
        dismiss.set_halign(gtk::Align::End);
        dismiss.set_margin_top(2);
        dismiss.set_margin_end(2);
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
        card.add_overlay(&dismiss);

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
                &dismiss,
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
                &dismiss,
            );
        }

        row.append(&card);
    }
}

/// Probes each path in an idle; [card_data_list] is DB-only (no libmpv) on the main thread, then
/// [schedule_thumb_backfill] for missing cache entries.
pub fn fill_idle(
    row: &gtk::Box,
    paths: Vec<std::path::PathBuf>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
) {
    let row = row.clone();
    let o = on_open;
    let r = on_remove;
    let _ = glib::idle_add_local(move || {
        eprintln!(
            "[rhino] recent: fill_idle build grid for {} path(s):",
            paths.len()
        );
        for p in &paths {
            eprintln!("[rhino] recent:   candidate {}", p.display());
        }
        let n = ensure_recent_backfill(&backfill, &row, o.clone(), r.clone());
        let v: Vec<CardData> = card_data_list(&paths);
        eprintln!("[rhino] recent: card_data done ({} cards)", v.len());
        for cd in &v {
            eprintln!(
                "[rhino] recent:   card path={} missing={}",
                cd.path.display(),
                cd.missing
            );
        }
        fill_row(&row, v, o.clone(), r.clone());
        let paths_t = paths.clone();
        schedule_thumb_backfill(n, paths_t);
        glib::ControlFlow::Break
    });
}
