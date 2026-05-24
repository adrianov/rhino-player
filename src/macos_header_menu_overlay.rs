//! Fullscreen theater: header menus in [`gtk::Overlay`] (no gdk-macos popup surface).

use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

include!("macos_header_menu_overlay_place.rs");

const BTN_OPEN_CLASS: &str = "rp-header-menu-open";
const FS_MENU_CLASS: &str = "rp-header-menu-fs";

fn set_btn_open(btn: &gtk::MenuButton, open: bool) {
    if open {
        btn.add_css_class(BTN_OPEN_CLASS);
    } else {
        btn.remove_css_class(BTN_OPEN_CLASS);
    }
}

struct MenuEntry {
    name: &'static str,
    btn: gtk::MenuButton,
    pop: gtk::Popover,
    pop_ph: gtk::Box,
}

fn set_fs_menu_btn(btn: &gtk::MenuButton, fs: bool) {
    if fs {
        btn.add_css_class(FS_MENU_CLASS);
    } else {
        btn.remove_css_class(FS_MENU_CLASS);
    }
}

fn detach_popovers(entries: &[MenuEntry]) {
    for e in entries {
        e.pop.popdown();
        e.btn.set_popover(None::<&gtk::Popover>);
        set_fs_menu_btn(&e.btn, true);
    }
}

fn attach_popovers(entries: &[MenuEntry]) {
    for e in entries {
        e.btn.set_popover(Some(&e.pop));
        set_fs_menu_btn(&e.btn, false);
    }
}

fn new_pop_placeholder() -> gtk::Box {
    let ph = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    ph.set_size_request(0, 0);
    ph.set_opacity(0.0);
    ph.set_can_target(false);
    ph.set_visible(false);
    ph
}

pub struct HeaderMenuOverlay {
    shell: gtk::Overlay,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    win: adw::ApplicationWindow,
    panel: gtk::Frame,
    entries: Vec<MenuEntry>,
    open: Cell<Option<usize>>,
}

thread_local! {
    static OVERLAY: RefCell<Option<Rc<HeaderMenuOverlay>>> = const { RefCell::new(None) };
}

pub fn register_overlay(ov: Rc<HeaderMenuOverlay>) {
    OVERLAY.with(|s| *s.borrow_mut() = Some(ov));
}

pub fn raise_overlay_child(shell: &gtk::Overlay, w: &impl IsA<gtk::Widget>) {
    raise_overlay_top(shell, w);
}

pub fn overlay_visible() -> bool {
    OVERLAY.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|o| o.panel.is_visible())
    })
}

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

include!("macos_header_menu_overlay_input.rs");

impl HeaderMenuOverlay {
    pub fn wire(
        shell: gtk::Overlay,
        win: adw::ApplicationWindow,
        root: adw::ToolbarView,
        header: adw::HeaderBar,
        menus: &[(gtk::MenuButton, gtk::Popover, &'static str)],
    ) -> Rc<Self> {
        let panel = gtk::Frame::new(None);
        attach_panel_css(&panel);
        panel.set_visible(false);
        panel.set_can_target(false);
        panel.set_hexpand(false);
        panel.set_vexpand(false);
        shell.add_overlay(&panel);

        let entries: Vec<MenuEntry> = menus
            .iter()
            .map(|(btn, pop, name)| MenuEntry {
                name,
                btn: btn.clone(),
                pop: pop.clone(),
                pop_ph: new_pop_placeholder(),
            })
            .collect();

        let ov = Rc::new(Self {
            shell,
            root: root.clone(),
            header: header.clone(),
            win: win.clone(),
            panel,
            entries,
            open: Cell::new(None),
        });

        let ov_hdr = Rc::clone(&ov);
        use glib::object::ObjectExt;
        header.connect_notify_local(Some("height"), move |_, _| {
            ov_hdr.reposition_open();
        });

        for (idx, entry) in ov.entries.iter().enumerate() {
            wire_btn_fullscreen_block(&win, &entry.btn);
            wire_popover_fullscreen_guard(&win, &entry.pop);
            wire_btn_press(Rc::clone(&ov), idx, entry);
        }

        let ov_fs = Rc::clone(&ov);
        win.connect_fullscreened_notify(move |w| {
            if w.is_fullscreen() {
                ov_fs.on_enter_fullscreen();
            } else {
                ov_fs.on_leave_fullscreen();
            }
        });

        register_overlay(Rc::clone(&ov));
        if win.is_fullscreen() {
            ov.on_enter_fullscreen();
        }
        ov
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

    fn toggle(&self, idx: usize) {
        if !self.win.is_fullscreen() {
            return;
        }
        if self.open.get() == Some(idx) {
            self.hide_panel();
            return;
        }
        self.hide_panel();
        let entry = &self.entries[idx];
        let Some(child) = entry.pop.child() else {
            return;
        };
        crate::macos_header_menu_debug::log_event(entry.name, "open", "reason=overlay");
        prep_fs_menu_layout(&self.root, &self.header, &self.shell);
        if entry.name == "audio" {
            crate::header_menu_tracks::refresh_audio_on_open();
        } else if entry.name == "subtitles" {
            crate::header_menu_tracks::refresh_sub_on_open();
        }
        entry.pop.set_child(Some(&entry.pop_ph));
        entry.pop.popdown();
        self.panel.set_child(Some(&child));
        prep_overlay_child(&child);
        enable_target_tree(&child);
        crate::macos_header_menu::attach_opaque_widget(&child);
        if entry.name == "speed" {
            if let Some(list) = find_list_box(&child) {
                crate::macos_header_menu::arm_list_pick_on_open(&list);
            }
        }
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

pub fn overlay_contains(widget: &gtk::Widget) -> bool {
    OVERLAY.with(|s| {
        let guard = s.borrow();
        let Some(ov) = guard.as_ref() else {
            return false;
        };
        if !ov.panel.is_visible() {
            return false;
        }
        let mut w = Some(widget.clone());
        while let Some(cur) = w {
            if cur == ov.panel {
                return true;
            }
            w = cur.parent();
        }
        false
    })
}
