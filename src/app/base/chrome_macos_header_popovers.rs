// macOS: non-modal header Popovers often ignore outside presses and same-button toggle.
// Explicit capture-phase dismissal matches Linux autohide behavior.

/// Close sound / subtitles / speed header menus (and their popovers).
fn close_header_menus(menus: &[gtk::MenuButton]) {
    for btn in menus {
        if let Some(pop) = btn.popover() {
            pop.popdown();
        }
        btn.set_active(false);
    }
}

fn widget_is_descendant_of(widget: &gtk::Widget, ancestor: &gtk::Widget) -> bool {
    let mut w = Some(widget.clone());
    while let Some(cur) = w {
        if cur == *ancestor {
            return true;
        }
        w = cur.parent();
    }
    false
}

fn click_targets_header_menu(
    picked: Option<gtk::Widget>,
    menus: &[gtk::MenuButton],
) -> bool {
    let Some(picked) = picked else {
        return false;
    };
    for btn in menus {
        if widget_is_descendant_of(&picked, btn.upcast_ref()) {
            return true;
        }
        if let Some(pop) = btn.popover() {
            if pop.is_visible() && widget_is_descendant_of(&picked, pop.upcast_ref()) {
                return true;
            }
        }
    }
    false
}

fn header_menu_open(menus: &[gtk::MenuButton]) -> bool {
    menus.iter().any(gtk::MenuButton::is_active)
        || menus.iter().filter_map(gtk::MenuButton::popover).any(|p| p.is_visible())
}

/// Dismiss open header popovers when the user clicks elsewhere in the shell.
pub(super) fn wire_macos_header_popover_dismiss(
    root: &impl IsA<gtk::Widget>,
    menus: &[gtk::MenuButton],
) {
    let root = root.clone().upcast::<gtk::Widget>();
    let menus: Vec<gtk::MenuButton> = menus.to_vec();
    let g = gtk::GestureClick::new();
    g.set_button(gtk::gdk::BUTTON_PRIMARY);
    g.set_propagation_phase(gtk::PropagationPhase::Capture);
    let root_pick = root.clone();
    g.connect_pressed(move |_, _n, x, y| {
        if !header_menu_open(&menus) {
            return;
        }
        let picked = root_pick.pick(x, y, gtk::PickFlags::DEFAULT);
        if click_targets_header_menu(picked, &menus) {
            return;
        }
        close_header_menus(&menus);
    });
    root.add_controller(g);
}
