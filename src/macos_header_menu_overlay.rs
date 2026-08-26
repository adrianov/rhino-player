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

pub fn overlay_visible() -> bool {
    OVERLAY.with(|s| s.borrow().as_ref().is_some_and(|o| o.panel.is_visible()))
}

include!("macos_header_menu_overlay_input.rs");
include!("macos_header_menu_overlay_panel.rs");

impl HeaderMenuOverlay {
    pub fn wire(
        shell: gtk::Overlay,
        win: adw::ApplicationWindow,
        root: adw::ToolbarView,
        header: adw::HeaderBar,
        menus: &[(gtk::MenuButton, gtk::Popover, &'static str)],
    ) -> Rc<Self> {
        let panel = Self::build_panel(&shell);
        let entries = Self::collect_entries(menus);

        let ov = Rc::new(Self {
            shell,
            root: root.clone(),
            header: header.clone(),
            win: win.clone(),
            panel,
            entries,
            open: Cell::new(None),
        });

        Self::track_header_height(&header, &ov);
        Self::wire_entry_controls(&win, &ov);
        Self::track_fullscreen(&win, &ov);

        register_overlay(Rc::clone(&ov));
        if win.is_fullscreen() {
            ov.on_enter_fullscreen();
        }
        ov
    }

    fn build_panel(shell: &gtk::Overlay) -> gtk::Frame {
        let panel = gtk::Frame::new(None);
        attach_panel_css(&panel);
        panel.set_visible(false);
        panel.set_can_target(false);
        panel.set_hexpand(false);
        panel.set_vexpand(false);
        shell.add_overlay(&panel);
        panel
    }

    fn collect_entries(menus: &[(gtk::MenuButton, gtk::Popover, &'static str)]) -> Vec<MenuEntry> {
        menus
            .iter()
            .map(|(btn, pop, name)| MenuEntry {
                name,
                btn: btn.clone(),
                pop: pop.clone(),
                pop_ph: new_pop_placeholder(),
            })
            .collect()
    }

    /// Re-anchor the open panel whenever the header bar height changes.
    fn track_header_height(header: &adw::HeaderBar, ov: &Rc<Self>) {
        use glib::object::ObjectExt;
        let ov_hdr = Rc::clone(ov);
        header.connect_notify_local(Some("height"), move |_, _| {
            ov_hdr.reposition_open();
        });
    }

    fn wire_entry_controls(win: &adw::ApplicationWindow, ov: &Rc<Self>) {
        for (idx, entry) in ov.entries.iter().enumerate() {
            wire_btn_fullscreen_block(win, &entry.btn);
            wire_popover_fullscreen_guard(win, &entry.pop);
            wire_btn_press(Rc::clone(ov), idx, entry);
        }
    }

    fn track_fullscreen(win: &adw::ApplicationWindow, ov: &Rc<Self>) {
        let ov_fs = Rc::clone(ov);
        win.connect_fullscreened_notify(move |w| {
            if w.is_fullscreen() {
                ov_fs.on_enter_fullscreen();
            } else {
                ov_fs.on_leave_fullscreen();
            }
        });
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
