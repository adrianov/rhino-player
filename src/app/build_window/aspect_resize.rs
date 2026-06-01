/// Builds the debounced resize handler that snaps window aspect after a drag ends.
fn build_aspect_on_resize(
    win: &adw::ApplicationWindow,
    recent: &gtk::Box,
    win_aspect: &Rc<WinAspectCell>,
    deb: &Rc<RefCell<Option<glib::SourceId>>>,
) -> Rc<dyn Fn()> {
    use gtk::prelude::WidgetExt;
    let win = win.clone();
    let recent = recent.clone();
    let win_aspect = Rc::clone(win_aspect);
    let deb = Rc::clone(deb);
    Rc::new(move || {
        let ww = win.width();
        let hh = win.height();
        if !resize_notify_changed(ww, hh) {
            return;
        }
        if aspect_debug() {
            eprintln!("[rhino] aspect: resize notify {ww}×{hh}");
        }
        schedule_window_aspect_on_resize_end(Rc::clone(&deb), &win, &recent, &win_aspect);
    })
}

/// Connects `on_resize` to the live `GdkSurface` width/height notify. Returns `false` when the
/// window has no surface yet (caller retries on the next idle).
fn connect_surface_resize(win: &adw::ApplicationWindow, on_resize: &Rc<dyn Fn()>) -> bool {
    use gtk::gdk::prelude::SurfaceExt;
    use gtk::prelude::NativeExt;
    let Some(surf) = win.native().and_then(|n| n.surface()) else {
        return false;
    };
    let f = Rc::clone(on_resize);
    surf.connect_width_notify(move |_| f());
    let f2 = Rc::clone(on_resize);
    surf.connect_height_notify(move |_| f2());
    true
}

/// Wires window + surface resize notify once (idempotent via `wired`).
fn install_aspect_hooks(
    win: &adw::ApplicationWindow,
    recent: &gtk::Box,
    win_aspect: &Rc<WinAspectCell>,
    deb: &Rc<RefCell<Option<glib::SourceId>>>,
    wired: &Rc<Cell<bool>>,
) {
    use glib::object::ObjectExt;
    if wired.replace(true) {
        return;
    }
    let on_resize = build_aspect_on_resize(win, recent, win_aspect, deb);
    win.connect_notify_local(Some("width"), {
        let f = Rc::clone(&on_resize);
        move |_, _| f()
    });
    win.connect_notify_local(Some("height"), {
        let f = Rc::clone(&on_resize);
        move |_, _| f()
    });

    let has_surf = connect_surface_resize(win, &on_resize);
    if !has_surf {
        let win_idle = win.clone();
        let on_idle = Rc::clone(&on_resize);
        let _ = glib::idle_add_local_once(move || log_deferred_surface_wire(&win_idle, &on_idle));
    }
    eprintln!(
        "[rhino] aspect: resize hooks wired (window notify{})",
        if has_surf { " + GdkSurface" } else { ", surface pending" }
    );
}

/// Idle retry: connect the surface notify once it exists, logging the outcome.
fn log_deferred_surface_wire(win: &adw::ApplicationWindow, on_resize: &Rc<dyn Fn()>) {
    use gtk::prelude::WidgetExt;
    if connect_surface_resize(win, on_resize) {
        eprintln!("[rhino] aspect: resize hooks GdkSurface (deferred)");
    } else {
        eprintln!(
            "[rhino] aspect: resize hooks window only (no GdkSurface {}×{})",
            win.width(),
            win.height()
        );
    }
}

/// Wires resize signals for post-drag aspect snap (safe if the window mapped before mpv realize).
fn wire_aspect_resize_on_map(
    win: &adw::ApplicationWindow,
    recent: &gtk::Box,
    win_aspect: &Rc<WinAspectCell>,
    deb: &Rc<RefCell<Option<glib::SourceId>>>,
    wired: &Rc<Cell<bool>>,
) {
    use gtk::prelude::WidgetExt;

    // [wire_window_after_present] runs after [present]; [connect_map] alone never fires.
    if win.is_mapped() {
        install_aspect_hooks(win, recent, win_aspect, deb, wired);
        return;
    }
    let win_c = win.clone();
    let recent = recent.clone();
    let win_aspect = Rc::clone(win_aspect);
    let deb = Rc::clone(deb);
    let wired = Rc::clone(wired);
    win.connect_map(move |_| {
        install_aspect_hooks(&win_c, &recent, &win_aspect, &deb, &wired);
    });
}
