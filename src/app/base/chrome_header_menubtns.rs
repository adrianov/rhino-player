/// No built-in “menu button group.” Before the [gtk::MenuButton] default: close other menus,
/// then an idle [set_active] if the first press did not open the target (e.g. lost to popover stack).
fn ensure_active_idle(btn: gtk::MenuButton) {
    glib::idle_add_local(move || {
        if !btn.is_active() {
            btn.set_active(true);
        }
        glib::ControlFlow::Break
    });
}

#[cfg(target_os = "macos")]
fn wire_macos_header_menu_cluster(
    root: &adw::ToolbarView,
    header: &adw::HeaderBar,
    shell: &gtk::Overlay,
    win: &adw::ApplicationWindow,
    entries: &[(gtk::MenuButton, gtk::Popover, &'static str)],
) {
    let menus: Vec<gtk::MenuButton> = entries.iter().map(|(b, _, _)| b.clone()).collect();
    header_menubtns_switch(&menus);
    wire_macos_header_popover_dismiss(shell, &menus);
    crate::macos_header_menu_overlay::HeaderMenuOverlay::wire(
        shell.clone(),
        win.clone(),
        root.clone(),
        header.clone(),
        entries,
    );
}

#[cfg(target_os = "macos")]
fn log_sibling_menu_close(sibs: &[gtk::MenuButton]) {
    let any = sibs.iter().any(|b| {
        b.is_active() || b.popover().is_some_and(|p| p.is_visible())
    });
    if any {
        crate::macos_header_menu_debug::log_event("header", "close", "reason=sibling_switch");
    }
}

#[cfg(target_os = "macos")]
fn header_menu_fullscreen(btn: &gtk::MenuButton) -> bool {
    btn.root()
        .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
        .is_some_and(|w| w.is_fullscreen())
}

fn header_menubtns_switch(menus: &[gtk::MenuButton]) {
    for (i, menu) in menus.iter().enumerate() {
        let g = gtk::GestureClick::new();
        g.set_button(gtk::gdk::BUTTON_PRIMARY);
        g.set_propagation_limit(gtk::PropagationLimit::None);
        g.set_propagation_phase(gtk::PropagationPhase::Capture);
        let this = menu.clone();
        let sibs: Vec<gtk::MenuButton> = menus
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i)
            .map(|(_, b)| b.clone())
            .collect();
        let c = this.clone();
        g.connect_pressed(move |_, n, _, _| {
            if n != 1 {
                return;
            }
            #[cfg(target_os = "macos")]
            if header_menu_fullscreen(&c) {
                return;
            }
            let had_other = sibs.iter().any(|b| b.is_active());
            #[cfg(target_os = "macos")]
            log_sibling_menu_close(&sibs);
            for b in &sibs {
                if let Some(pop) = b.popover() {
                    pop.popdown();
                }
                b.set_active(false);
            }
            if had_other && !c.is_active() {
                ensure_active_idle(c.clone());
            }
        });
        this.add_controller(g);
    }
}
