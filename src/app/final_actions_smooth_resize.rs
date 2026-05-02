// GTK allocation / scale-factor hooks for subtitle toolbar lift.

fn wire_smooth_resize_and_subtitle_pos(
    gl: &gtk::GLArea,
    bottom: &gtk::Box,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    bar_show: &Rc<Cell<bool>>,
    recent: &gtk::Box,
) {
    let pz = Rc::clone(player);
    let bz = Rc::clone(bar_show);
    let rz = recent.clone();
    let botz = bottom.clone();
    let glz = gl.clone();
    let on_sz = Rc::new(move || {
        if let Some(b) = pz.borrow().as_ref() {
            let show = if rz.is_visible() { true } else { bz.get() };
            sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, botz.height(), glz.height());
        }
    });
    gl.connect_notify_local(Some("height"), glib::clone!(
        #[strong]
        on_sz,
        move |_, _| on_sz()
    ));
    gl.connect_notify_local(Some("width"), glib::clone!(
        #[strong]
        on_sz,
        move |_, _| on_sz()
    ));
    gl.connect_notify_local(Some("scale-factor"), glib::clone!(
        #[strong]
        on_sz,
        move |_, _| on_sz()
    ));
    let sub_bottom = Rc::clone(&on_sz);
    bottom.connect_notify_local(Some("height"), move |_, _| sub_bottom());
}
