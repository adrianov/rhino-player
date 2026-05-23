// macOS: non-modal header Popovers often ignore outside presses and same-button toggle.

/// Close sound / subtitles / speed header popovers.
pub(super) fn popdown_header_menus(menus: &[gtk::MenuButton], reason: &str) {
    crate::macos_header_menu::popdown_all(menus, reason);
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
    crate::macos_header_menu_overlay::overlay_contains(&picked)
}

fn header_menu_open(menus: &[gtk::MenuButton]) -> bool {
    crate::macos_header_menu::any_open()
        || crate::macos_header_menu_overlay::overlay_visible()
        || menus.iter().any(gtk::MenuButton::is_active)
}

/// Dismiss open header popovers when the user clicks elsewhere in the shell.
pub(super) fn wire_macos_header_popover_dismiss(
    shell: &impl IsA<gtk::Widget>,
    menus: &[gtk::MenuButton],
) {
    use gtk::prelude::GestureSingleExt;
    let shell = shell.clone().upcast::<gtk::Widget>();
    let menus: Vec<gtk::MenuButton> = menus.to_vec();
    let g = gtk::GestureClick::new();
    g.set_button(gtk::gdk::BUTTON_PRIMARY);
    g.set_propagation_phase(gtk::PropagationPhase::Capture);
    let shell_pick = shell.clone();
    g.connect_pressed(move |_, _n, x, y| {
        if !crate::macos_header_menu::dismiss_allowed() {
            return;
        }
        if !header_menu_open(&menus) {
            return;
        }
        let picked = shell_pick.pick(x, y, gtk::PickFlags::DEFAULT);
        if click_targets_header_menu(picked.clone(), &menus) {
            return;
        }
        #[cfg(target_os = "macos")]
        crate::macos_header_menu_debug::log_event(
            "header",
            "close",
            &format!("reason=outside_click pick={}", picked.is_some()),
        );
        popdown_header_menus(&menus, "outside_click");
    });
    shell.add_controller(g);
}
