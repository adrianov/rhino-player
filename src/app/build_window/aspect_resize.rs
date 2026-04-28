/// Wires surface and window resize signals (once, on first map) to trigger
/// aspect-ratio–aware window resize scheduling.
fn wire_aspect_resize_on_map(
    win: &adw::ApplicationWindow,
    recent: &gtk::ScrolledWindow,
    win_aspect: &Rc<Cell<Option<f64>>>,
    deb: &Rc<RefCell<Option<glib::SourceId>>>,
    wired: &Rc<Cell<bool>>,
) {
    win.connect_map(glib::clone!(
        #[strong]
        win,
        #[strong]
        recent,
        #[strong]
        win_aspect,
        #[strong]
        deb,
        #[strong]
        wired,
        move |_| {
            if wired.get() {
                return;
            }
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
                    schedule_window_aspect_on_resize_end(Rc::clone(&deb), &win, &recent, &win_aspect)
                }
            ));
            let Some(n) = win.native() else {
                return;
            };
            let Some(surf) = n.surface() else {
                return;
            };
            surf.connect_width_notify(glib::clone!(
                #[strong]
                on_resize,
                move |_| on_resize()
            ));
            surf.connect_height_notify(glib::clone!(
                #[strong]
                on_resize,
                move |_| on_resize()
            ));
            let gw: &gtk::Window = win.upcast_ref();
            gw.connect_default_width_notify(glib::clone!(
                #[strong]
                on_resize,
                move |_| on_resize()
            ));
            gw.connect_default_height_notify(glib::clone!(
                #[strong]
                on_resize,
                move |_| on_resize()
            ));
            wired.set(true);
            if aspect_debug() {
                eprintln!(
                    "[rhino] aspect: resize-end hooks (GdkSurface + GtkWindow default size)"
                );
            }
        }
    ));
}
