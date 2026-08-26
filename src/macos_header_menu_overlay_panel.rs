// Fullscreen header menu overlay: panel open/close lifecycle and fullscreen transitions.

pub fn overlay_close_all(reason: &str) {
    let Some(ov) = OVERLAY.with(|s| s.borrow().clone()) else {
        return;
    };
    if ov.panel.is_visible() {
        crate::macos_header_menu_debug::log_event("header", "close", &format!("reason={reason}"));
    }
    ov.hide_panel();
}

pub fn clear_btn_open(menus: &[gtk::MenuButton]) {
    for btn in menus {
        set_btn_open(btn, false);
    }
}

fn attach_panel_css(panel: &gtk::Frame) {
    panel.add_css_class("rp-header-popover");
    panel.add_css_class("rp-header-menu-overlay");
    crate::macos_header_menu::attach_opaque_widget(panel.upcast_ref());
}

fn refresh_tracks_on_open(name: &str) {
    if name == "audio" {
        crate::header_menu_tracks::refresh_audio_on_open();
    } else if name == "subtitles" {
        crate::header_menu_tracks::refresh_sub_on_open();
    }
}

/// Prepare a popover child to live directly on the overlay panel.
fn stage_overlay_child(child: &gtk::Widget) {
    prep_overlay_child(child);
    enable_target_tree(child);
    crate::macos_header_menu::attach_opaque_widget(child);
}

fn arm_speed_list_if_needed(name: &str, child: &gtk::Widget) {
    if name != "speed" {
        return;
    }
    if let Some(list) = find_list_box(child) {
        crate::macos_header_menu::arm_list_pick_on_open(&list);
    }
}

impl HeaderMenuOverlay {
    fn hide_panel(&self) {
        let Some(idx) = self.open.take() else {
            hide_panel_widget(&self.panel);
            return;
        };
        let entry = &self.entries[idx];
        set_btn_open(&entry.btn, false);
        entry.pop.popdown();
        if let Some(child) = self.panel.child() {
            reset_scrolled_heights(&child);
            self.panel.set_child(None::<&gtk::Widget>);
            entry.pop.set_child(Some(&child));
        }
        hide_panel_widget(&self.panel);
        crate::macos_header_menu::on_menu_surface_closed();
    }

    fn reposition_open(&self) {
        let Some(idx) = self.open.get() else {
            return;
        };
        if !self.panel.is_visible() || !self.win.is_fullscreen() {
            return;
        }
        prep_fs_menu_layout(&self.root, &self.header, &self.shell);
        place_panel_clamped(&self.panel, &self.entries[idx].btn, &self.shell);
    }

    fn on_enter_fullscreen(&self) {
        self.hide_panel();
        for e in &self.entries {
            set_btn_open(&e.btn, false);
            e.btn.set_active(false);
        }
        detach_popovers(&self.entries);
    }

    fn on_leave_fullscreen(&self) {
        self.hide_panel();
        attach_popovers(&self.entries);
        for e in &self.entries {
            set_btn_open(&e.btn, false);
            e.btn.set_active(false);
        }
    }

    fn toggle(&self, idx: usize) {
        if !self.win.is_fullscreen() {
            return;
        }
        if self.open.get() == Some(idx) {
            self.hide_panel();
            return;
        }
        self.hide_panel();
        self.open_panel(idx);
    }

    /// Steal the popover's child onto the overlay panel and show it under the button.
    fn open_panel(&self, idx: usize) {
        let entry = &self.entries[idx];
        let Some(child) = entry.pop.child() else {
            return;
        };
        crate::macos_header_menu_debug::log_event(entry.name, "open", "reason=overlay");
        prep_fs_menu_layout(&self.root, &self.header, &self.shell);
        refresh_tracks_on_open(entry.name);
        entry.pop.set_child(Some(&entry.pop_ph));
        entry.pop.popdown();
        self.panel.set_child(Some(&child));
        stage_overlay_child(&child);
        arm_speed_list_if_needed(entry.name, &child);
        self.finish_open(idx, entry);
    }

    fn finish_open(&self, idx: usize, entry: &MenuEntry) {
        place_panel_clamped(&self.panel, &entry.btn, &self.shell);
        self.open.set(Some(idx));
        set_btn_open(&entry.btn, true);
        show_panel(&self.panel, &self.shell);
        self.panel.queue_allocate();
        crate::macos_header_menu::on_overlay_surface_opened();
    }

    fn close_siblings(&self, keep: usize) {
        for (i, e) in self.entries.iter().enumerate() {
            if i != keep {
                set_btn_open(&e.btn, false);
                e.btn.set_active(false);
                e.pop.popdown();
            }
        }
        if self.open.get().is_some_and(|i| i != keep) {
            self.hide_panel();
        }
    }
}
