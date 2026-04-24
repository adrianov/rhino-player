use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use crate::format_time;
use crate::mpv_embed::MpvBundle;
use crate::theme;

const APP_ID: &str = "ch.rhino.RhinoPlayer";

pub fn run() -> i32 {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }

    if let Err(e) = adw::init() {
        eprintln!("libadwaita: {e}");
        return 1;
    }

    let app = adw::Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_app| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        theme::apply();
    });

    let player: Rc<RefCell<Option<MpvBundle>>> = Rc::new(RefCell::new(None));

    {
        let p = player.clone();
        app.connect_activate(move |a: &adw::Application| {
            if a.windows().is_empty() {
                build_window(a, &p);
            }
        });
    }

    app.run().into()
}

fn build_window(app: &adw::Application, player: &Rc<RefCell<Option<MpvBundle>>>) {
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title("Rhino Player")
        .default_width(960)
        .default_height(540)
        .css_classes(["rp-win"])
        .build();

    let root = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    header.add_css_class("rpb-header");
    let menu = gio::Menu::new();
    menu.append(Some("Open…"), Some("app.open"));
    menu.append(Some("About Rhino Player"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));
    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.set_tooltip_text(Some("Main menu"));
    menu_btn.set_menu_model(Some(&menu));
    header.pack_end(&menu_btn);

    let status = gtk::Label::new(Some("Initializing video…"));
    status.add_css_class("rp-status");
    status.set_wrap(true);
    status.set_wrap_mode(gtk::pango::WrapMode::Word);
    status.set_xalign(0.0);
    status.set_vexpand(false);

    let gl_area = gtk::GLArea::new();
    gl_area.add_css_class("rp-gl");
    gl_area.set_hexpand(true);
    gl_area.set_vexpand(true);
    gl_area.set_auto_render(false);
    gl_area.set_has_stencil_buffer(false);
    gl_area.set_has_depth_buffer(false);

    let seek_adj = gtk::Adjustment::new(0.0, 0.0, 1.0, 0.2, 1.0, 0.0);
    let seek = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&seek_adj));
    seek.set_hexpand(true);
    seek.set_draw_value(false);
    seek.set_sensitive(false);
    seek.add_css_class("rp-seek");
    seek.set_size_request(120, 0);
    let time_left = gtk::Label::new(Some("0:00"));
    time_left.add_css_class("rp-time");
    time_left.set_xalign(0.0);
    let time_right = gtk::Label::new(Some("0:00"));
    time_right.set_css_classes(&["rp-time", "rp-time-dim"]);
    time_right.set_xalign(1.0);

    let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bottom.add_css_class("rp-bottom");
    bottom.set_vexpand(false);
    bottom.append(&time_left);
    bottom.append(&seek);
    bottom.append(&time_right);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    vbox.add_css_class("rp-stack");
    vbox.append(&status);
    vbox.append(&gl_area);

    root.add_top_bar(&header);
    root.set_content(Some(&vbox));
    root.add_bottom_bar(&bottom);

    win.set_content(Some(&root));

    let p_realize = player.clone();
    let st_realize = status.clone();
    gl_area.connect_realize(move |area| {
        area.make_current();
        match MpvBundle::new(area) {
            Ok(b) => {
                *p_realize.borrow_mut() = Some(b);
                st_realize.set_label("Open a file (Ctrl+O).");
            }
            Err(e) => st_realize.set_label(&format!("OpenGL / mpv: {e}")),
        }
    });

    let p_draw = player.clone();
    gl_area.connect_render(move |area, _ctx| {
        area.make_current();
        if let Some(b) = p_draw.borrow().as_ref() {
            b.draw(area);
        }
        glib::Propagation::Stop
    });

    let seek_sync = Rc::new(Cell::new(false));
    let p_seek = player.clone();
    seek.connect_value_changed(glib::clone!(
        #[strong]
        p_seek,
        #[strong]
        seek_sync,
        move |r| {
            if seek_sync.get() {
                return;
            }
            if let Some(b) = p_seek.borrow().as_ref() {
                let _ = b.mpv.set_property("time-pos", r.value());
            }
        }
    ));

    let p_poll = player.clone();
    let s_flag = seek_sync.clone();
    let tw_l = time_left.downgrade();
    let tw_r = time_right.downgrade();
    let sw = seek.clone();
    let adj = seek_adj.clone();
    glib::timeout_add_local(
        Duration::from_millis(200),
        glib::clone!(
            #[strong]
            p_poll,
            move || {
                let Some(tl) = tw_l.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let Some(tr) = tw_r.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let g = p_poll.borrow();
                let Some(pl) = g.as_ref() else {
                    return glib::ControlFlow::Continue;
                };
                let pos = pl.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
                let dur = pl.mpv.get_property::<f64>("duration").unwrap_or(0.0);
                tl.set_label(&format_time(pos));
                tr.set_label(&format_time(dur));
                if dur > 0.0 {
                    sw.set_sensitive(true);
                    adj.set_lower(0.0);
                    adj.set_upper(dur);
                    s_flag.set(true);
                    adj.set_value(pos.clamp(0.0, dur));
                    s_flag.set(false);
                } else {
                    sw.set_sensitive(false);
                }
                glib::ControlFlow::Continue
            }
        ),
    );

    // Open
    let open = gio::SimpleAction::new("open", None);
    let p_open = player.clone();
    let st = status.clone();
    let gl_w = gl_area.clone();
    open.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let Some(w) = app.active_window() else { return; };
            let dialog = gtk::FileDialog::builder()
                .title("Open media")
                .modal(true)
                .build();
            let p_c = p_open.clone();
            let st = st.clone();
            let gl_w = gl_w.clone();
            dialog.open(Some(&w), None::<&gio::Cancellable>, move |res| {
                let Ok(file) = res else { return; };
                let Some(path) = file.path() else {
                    st.set_label("Non-path URIs: not implemented yet.");
                    return;
                };
                let path_s = path.to_string_lossy();
                let mut g = p_c.borrow_mut();
                let Some(b) = g.as_mut() else {
                    st.set_label("Player not ready yet. Wait for GL init.");
                    return;
                };
                if let Err(e) = b.mpv.command("loadfile", &[path_s.as_ref(), "replace"]) {
                    st.set_label(&format!("loadfile: {e:?}"));
                    return;
                }
                st.set_label(&format!("Loaded: {}", path.display()));
                gl_w.queue_render();
            });
        }
    ));
    app.add_action(&open);

    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let parent = app.active_window();
            let mut b = gtk::AboutDialog::builder()
                .program_name("Rhino Player")
                .version(env!("CARGO_PKG_VERSION"))
                .comments("mpv with GTK 4 and libadwaita (ToolbarView: seek as bottom bar).")
                .license_type(gtk::License::Gpl30)
                .website("https://github.com/placeholder/rhino-player")
                .modal(true);
            if let Some(ref w) = parent {
                b = b.transient_for(w);
            }
            b.build().present();
        }
    ));
    app.add_action(&about);

    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            app.quit();
        }
    ));
    app.add_action(&quit);

    app.set_accels_for_action("app.open", &["<Primary>o"]);
    app.set_accels_for_action("app.about", &["F1"]);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);

    win.present();
}
