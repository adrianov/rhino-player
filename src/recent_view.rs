//! Recent video cards for empty launch. See [docs/features/21-recent-videos-launch.md].

use adw::prelude::*;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::EventControllerExt;
use std::path::Path;
use std::rc::Rc;

use crate::media_probe::{card_data_list, CardData};

/// Scrolled, vertically and horizontally centered row of at most five cards.
pub fn new_scroll() -> (gtk::ScrolledWindow, gtk::Box) {
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
    v.append(&sp_top);
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
    (s, h)
}

fn clear(f: &gtk::Box) {
    while let Some(c) = f.first_child() {
        c.unparent();
    }
}

fn no_target(w: &impl IsA<gtk::Widget>) {
    w.set_can_target(false);
}

type UnitFn = Rc<dyn Fn(()) + 'static>;

/// Hand on hover, primary click triggers [act] ([GestureClick] on the card, not a nested [gtk::Button]).
/// Uses [connect_pressed] (not [GestureClick::connect_released]): [gtk::LevelBar] / scale-like
/// children can prevent a paired `released` in the same gesture, so the handler would never run.
fn add_click_and_pointer(card: &gtk::Box, debug_path: &str, act: UnitFn) {
    card.set_can_target(true);
    let g = gtk::GestureClick::new();
    g.set_button(1);
    g.set_propagation_phase(gtk::PropagationPhase::Capture);
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
    card.add_controller(g);

    let c = card.clone();
    let m = gtk::EventControllerMotion::new();
    m.connect_enter(move |_, _x, _y| {
        c.set_cursor_from_name(Some("pointer"));
    });
    let c = card.clone();
    m.connect_leave(move |_| {
        c.set_cursor_from_name(None);
    });
    card.add_controller(m);
}

/// Replace all children with cards; [on_open] / [on_stale] are only called from the main thread.
pub fn fill_row(
    row: &gtk::Box,
    items: Vec<CardData>,
    on_open: Rc<dyn Fn(&Path)>,
    on_stale: Rc<dyn Fn(&Path)>,
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

        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.set_size_request(200, -1);
        card.set_width_request(200);
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

        let pict = if miss {
            gtk::Image::from_icon_name("image-missing-symbolic")
        } else if let Some(ref png) = d.thumb {
            let b = glib::Bytes::from(png.as_slice());
            if let Ok(tex) = gdk::Texture::from_bytes(&b) {
                gtk::Image::from_paintable(Some(&tex))
            } else {
                gtk::Image::from_icon_name("video-x-generic")
            }
        } else {
            gtk::Image::from_icon_name("video-x-generic")
        };
        no_target(&pict);
        pict.add_css_class("rp-recent-pict");
        pict.set_size_request(200, 112);

        let label = gtk::Label::new(Some(&name));
        no_target(&label);
        label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        label.set_max_width_chars(24);
        label.set_tooltip_text(c.to_str());

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

        card.append(&pict);
        card.append(&label);
        card.append(&pro);

        if miss {
            let path = c.clone();
            let sl = on_stale.clone();
            add_click_and_pointer(
                &card,
                &c.display().to_string(),
                Rc::new(move |()| {
                    eprintln!(
                        "[rhino] recent: stale remove callback path={}",
                        path.display()
                    );
                    sl(&path);
                }),
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
            );
        }

        row.append(&card);
    }
}

/// Probes each path in an idle; [card_data_list] runs on the main thread.
pub fn fill_idle(
    row: &gtk::Box,
    paths: Vec<std::path::PathBuf>,
    on_open: Rc<dyn Fn(&Path)>,
    on_stale: Rc<dyn Fn(&Path)>,
) {
    let row = row.clone();
    let o = on_open;
    let s = on_stale;
    let _ = glib::idle_add_local(move || {
        eprintln!(
            "[rhino] recent: fill_idle build grid for {} path(s):",
            paths.len()
        );
        for p in &paths {
            eprintln!("[rhino] recent:   candidate {}", p.display());
        }
        let v: Vec<CardData> = card_data_list(&paths);
        eprintln!("[rhino] recent: card_data done ({} cards)", v.len());
        for d in &v {
            eprintln!(
                "[rhino] recent:   card path={} missing={}",
                d.path.display(),
                d.missing
            );
        }
        fill_row(&row, v, o.clone(), s.clone());
        glib::ControlFlow::Break
    });
}
