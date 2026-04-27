/// Keeps floating chrome visible while any header popover is open.
fn wire_menu_chrome(
    ch: Rc<ChromeBarHide>,
    vol: &gtk::MenuButton,
    sub: &gtk::MenuButton,
    speed: &gtk::MenuButton,
    main: &gtk::MenuButton,
) {
    let h = Rc::new(move || {
        let any =
            ch.vol.is_active() || ch.sub.is_active() || ch.speed.is_active() || ch.main.is_active();
        if any {
            if let Some(id) = ch.nav.borrow_mut().take() {
                id.remove();
            }
            if ch.bar_show.get() {
                return;
            }
            ch.bar_show.set(true);
            apply_chrome(
                &ch.root,
                &ch.gl,
                &ch.bar_show,
                &ch.recent,
                &ch.bottom,
                &ch.player,
            );
        } else {
            schedule_bars_autohide(Rc::clone(&ch));
        }
    });
    let h1 = Rc::clone(&h);
    let h2 = Rc::clone(&h);
    let h3 = Rc::clone(&h);
    let h4 = Rc::clone(&h);
    vol.connect_active_notify(move |_| h1());
    sub.connect_active_notify(move |_| h3());
    speed.connect_active_notify(move |_| h4());
    main.connect_active_notify(move |_| h2());
}
