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
        g.connect_pressed(move |gesture, n, _, _| {
            if n != 1 {
                return;
            }
            #[cfg(target_os = "macos")]
            if c.is_active() {
                if let Some(pop) = c.popover() {
                    pop.popdown();
                }
                c.set_active(false);
                let _ = gesture.set_state(gtk::EventSequenceState::Claimed);
                return;
            }
            let had_other = sibs.iter().any(|b| b.is_active());
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

#[cfg(target_os = "macos")]
fn wire_macos_header_menu_cluster(root: &adw::ToolbarView, menus: &[gtk::MenuButton]) {
    header_menubtns_switch(menus);
    wire_macos_header_popover_dismiss(root, menus);
}
