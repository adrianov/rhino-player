/// Wires resize signals for post-drag aspect snap (safe if the window mapped before mpv realize).
fn wire_aspect_resize_on_map(
    win: &adw::ApplicationWindow,
    recent: &gtk::Box,
    win_aspect: &Rc<WinAspectCell>,
    deb: &Rc<RefCell<Option<glib::SourceId>>>,
    wired: &Rc<Cell<bool>>,
) {
    use glib::object::ObjectExt;
    use gtk::prelude::WidgetExt;

    let install = {
        let win = win.clone();
        let recent = recent.clone();
        let win_aspect = Rc::clone(win_aspect);
        let deb = Rc::clone(deb);
        let wired = Rc::clone(wired);
        Rc::new(move || {
            if wired.get() {
                return;
            }
            wired.set(true);

            let on_resize: Rc<dyn Fn()> = Rc::new(glib::clone!(
                #[strong]
                deb,
                #[strong]
                win,
                #[strong]
                recent,
                #[strong]
                win_aspect,
                move || {
                    let ww = win.width();
                    let hh = win.height();
                    if !resize_notify_changed(ww, hh) {
                        return;
                    }
                    if aspect_debug() {
                        eprintln!("[rhino] aspect: resize notify {ww}×{hh}");
                    }
                    schedule_window_aspect_on_resize_end(
                        Rc::clone(&deb),
                        &win,
                        &recent,
                        &win_aspect,
                    );
                }
            ));

            win.connect_notify_local(Some("width"), {
                let f = Rc::clone(&on_resize);
                move |_, _| f()
            });
            win.connect_notify_local(Some("height"), {
                let f = Rc::clone(&on_resize);
                move |_, _| f()
            });

            let surface_done = Rc::new(Cell::new(false));
            let wire_surface = {
                let win_s = win.clone();
                let on_s = Rc::clone(&on_resize);
                let done = Rc::clone(&surface_done);
                move || {
                    if done.get() {
                        return true;
                    }
                    use gtk::gdk::prelude::SurfaceExt;
                    use gtk::prelude::NativeExt;
                    let Some(surf) = win_s.native().and_then(|n| n.surface()) else {
                        return false;
                    };
                    let f = Rc::clone(&on_s);
                    surf.connect_width_notify(move |_| f());
                    let f2 = Rc::clone(&on_s);
                    surf.connect_height_notify(move |_| f2());
                    done.set(true);
                    true
                }
            };

            let has_surf = wire_surface();
            if !has_surf {
                let win_idle = win.clone();
                let wire_idle = wire_surface;
                let _ = glib::idle_add_local_once(move || {
                    if wire_idle() {
                        eprintln!("[rhino] aspect: resize hooks GdkSurface (deferred)");
                    } else {
                        eprintln!(
                            "[rhino] aspect: resize hooks window only (no GdkSurface {}×{})",
                            win_idle.width(),
                            win_idle.height()
                        );
                    }
                });
            }

            eprintln!(
                "[rhino] aspect: resize hooks wired (window notify{})",
                if has_surf { " + GdkSurface" } else { ", surface pending" }
            );
        })
    };

    // [wire_window_after_present] runs after [present]; [connect_map] alone never fires.
    if win.is_mapped() {
        install();
    } else {
        let install_map = Rc::clone(&install);
        win.connect_map(move |_| install_map());
    }
}
