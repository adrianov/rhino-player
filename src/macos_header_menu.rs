//! macOS gdk-macos: standard [`gtk::MenuButton`] + [`gtk::Popover`] only.
//! Widget-level opaque CSS on map/show (display CSS is not enough over native video).
//! Fullscreen: popover surfaces can resize the Gdk surface; debounced compositing refresh
//! must not run [`invalidate_window_layers`] during open (windowed mode rarely hits this).
//! See `docs/references-gtk4-macos-header-menus.md`.

use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const MENU_HOLD_MS: u32 = 300;

thread_local! {
    static CHROME_HOLD: RefCell<Option<Rc<dyn Fn() -> bool>>> = const { RefCell::new(None) };
    static POP_VISIBLE: RefCell<Option<Rc<dyn Fn() -> bool>>> = const { RefCell::new(None) };
    static DISMISS_OK: Cell<bool> = const { Cell::new(true) };
    static COMPOSITING_ARMED: Cell<bool> = const { Cell::new(false) };
}

pub fn register_checks(chrome_hold: Rc<dyn Fn() -> bool>, pop_visible: Rc<dyn Fn() -> bool>) {
    CHROME_HOLD.with(|s| *s.borrow_mut() = Some(chrome_hold));
    POP_VISIBLE.with(|s| *s.borrow_mut() = Some(pop_visible));
}

/// Toolbar auto-hide / chrome: MenuButton active or popover surface visible.
pub fn any_open() -> bool {
    CHROME_HOLD.with(|s| s.borrow().as_ref().is_some_and(|f| f()))
}

fn pop_surface_visible() -> bool {
    POP_VISIBLE.with(|s| s.borrow().as_ref().is_some_and(|f| f()))
}

/// Skip layer invalidation during the brief open/arm window or while a popover popup exists.
pub fn defer_layer_invalidate() -> bool {
    pop_surface_visible() || COMPOSITING_ARMED.get()
}

pub fn dismiss_allowed() -> bool {
    DISMISS_OK.get()
}

fn pause_dismiss() {
    DISMISS_OK.set(false);
    let _ = glib::timeout_add_local_once(
        std::time::Duration::from_millis(u64::from(MENU_HOLD_MS)),
        || DISMISS_OK.set(true),
    );
}

fn arm_compositing_hold() {
    COMPOSITING_ARMED.set(true);
    let _ = glib::timeout_add_local_once(
        std::time::Duration::from_millis(u64::from(MENU_HOLD_MS)),
        || COMPOSITING_ARMED.set(false),
    );
}

fn disarm_compositing_hold() {
    COMPOSITING_ARMED.set(false);
}

pub fn arm_shell_compositing_hold() {
    arm_compositing_hold();
}

/// Fullscreen overlay opened (header menu or seek preview): queue shell repaint, then full refresh
/// after the compositing arm window (avoids stale gdk-macos header tiles on the video layer).
pub fn on_overlay_surface_opened() {
    arm_shell_compositing_hold();
    crate::app::refresh_registered_shell_compositing();
    let _ = glib::timeout_add_local_once(
        std::time::Duration::from_millis(u64::from(MENU_HOLD_MS) + 32),
        crate::app::refresh_registered_shell_compositing,
    );
}

pub fn on_header_menu_press(_btn: &gtk::MenuButton) {
    pause_dismiss();
    arm_compositing_hold();
}

/// Fullscreen overlay panel closed (same compositing tail as popover `closed`).
pub fn on_menu_surface_closed() {
    disarm_compositing_hold();
    schedule_compositing_refresh_after_menu();
}

fn schedule_compositing_refresh_after_menu() {
    let _ = glib::idle_add_local_once(|| {
        if !defer_layer_invalidate() {
            crate::app::refresh_registered_shell_compositing();
        }
    });
}

fn provider() -> &'static gtk::CssProvider {
    Box::leak(Box::new({
        let p = gtk::CssProvider::new();
        p.load_from_string(
            "popover.rp-header-popover > contents {\
                padding: 0;\
                background-color: #2d2d2d;\
                background: #2d2d2d;\
                opacity: 1;\
            }\
            box.rp-popover-box {\
                background-color: #2d2d2d;\
                background: #2d2d2d;\
                opacity: 1;\
            }\
            box.rp-header-menu-overlay,\
            frame.rp-header-menu-overlay {\
                background-color: #2d2d2d;\
                background: #2d2d2d;\
                opacity: 1;\
                border: none;\
                border-radius: 8px;\
                box-shadow: 0 8px 22px rgba(0, 0, 0, 0.45);\
            }",
        );
        p
    }))
}

/// Widget-level opaque chrome (popover body or fullscreen overlay panel).
pub fn attach_opaque_widget(w: &gtk::Widget) {
    attach_provider(w);
}

fn attach_provider(w: &gtk::Widget) {
    #[allow(deprecated)]
    gtk::prelude::StyleContextExt::add_provider(
        &w.style_context(),
        provider(),
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
}

fn paint_opaque(pop: &gtk::Popover) {
    attach_provider(pop.upcast_ref());
    if let Some(child) = pop.child() {
        attach_provider(&child);
    }
}

fn popover_in_fullscreen(p: &gtk::Popover) -> bool {
    p.root()
        .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
        .is_some_and(|w| w.is_fullscreen())
}

/// Opaque Adwaita popover chrome when gdk-macos maps the popover surface.
pub fn wire_popover(pop: &gtk::Popover) {
    let pop = pop.clone();
    pop.connect_map(move |p| {
        if popover_in_fullscreen(p) {
            p.popdown();
            return;
        }
        paint_opaque(p);
    });
    pop.connect_show(move |p| {
        if popover_in_fullscreen(p) {
            p.popdown();
            return;
        }
        paint_opaque(p);
        p.queue_draw();
        pause_dismiss();
        arm_compositing_hold();
    });
    pop.connect_closed(move |_| {
        disarm_compositing_hold();
        schedule_compositing_refresh_after_menu();
    });
}

/// Before the popover surface exists: block outside dismiss + compositing refresh.
pub fn wire_menu_btn_open_guard(btn: &gtk::MenuButton) {
    let btn = btn.clone();
    let g = gtk::GestureClick::new();
    g.set_button(gtk::gdk::BUTTON_PRIMARY);
    g.set_propagation_phase(gtk::PropagationPhase::Capture);
    let btn_press = btn.clone();
    g.connect_pressed(move |_, n, _, _| {
        if n == 1 {
            on_header_menu_press(&btn_press);
        }
    });
    btn.add_controller(g);
}

/// Speed list: block spurious selection while the opening click settles (theater toolbar).
pub fn arm_menu_list_pick_guard(pop: &gtk::Popover, list: &gtk::ListBox) -> Rc<Cell<bool>> {
    fn arm(block: &Rc<Cell<bool>>) {
        block.set(true);
        let b2 = block.clone();
        let _ = glib::timeout_add_local_once(
            std::time::Duration::from_millis(u64::from(MENU_HOLD_MS)),
            move || b2.set(false),
        );
    }
    fn freeze_list(list: &gtk::ListBox) {
        list.set_sensitive(false);
        let list = list.clone();
        let _ = glib::timeout_add_local_once(
            std::time::Duration::from_millis(u64::from(MENU_HOLD_MS)),
            move || list.set_sensitive(true),
        );
    }
    let block = Rc::new(Cell::new(false));
    let b_map = block.clone();
    let list_map = list.clone();
    pop.connect_map(move |_| {
        arm(&b_map);
        freeze_list(&list_map);
    });
    let b_show = block.clone();
    let list_show = list.clone();
    pop.connect_show(move |_| {
        arm(&b_show);
        freeze_list(&list_show);
    });
    block
}

thread_local! {
    static LIST_PICK: RefCell<Option<Rc<Cell<bool>>>> = const { RefCell::new(None) };
}

pub fn register_list_pick(block: Rc<Cell<bool>>) {
    LIST_PICK.with(|s| *s.borrow_mut() = Some(block));
}

fn arm_list_pick() {
    LIST_PICK.with(|s| {
        let guard = s.borrow();
        let Some(block) = guard.as_ref() else {
            return;
        };
        block.set(true);
        let b2 = block.clone();
        let _ = glib::timeout_add_local_once(
            std::time::Duration::from_millis(u64::from(MENU_HOLD_MS)),
            move || b2.set(false),
        );
    });
}

/// Fullscreen overlay open: same pick guard as popover map/show (windowed).
pub fn arm_list_pick_on_open(list: &gtk::ListBox) {
    list.set_sensitive(false);
    let list = list.clone();
    let _ = glib::timeout_add_local_once(
        std::time::Duration::from_millis(u64::from(MENU_HOLD_MS)),
        move || list.set_sensitive(true),
    );
    arm_list_pick();
}

pub fn popdown_all(menus: &[gtk::MenuButton], reason: &str) {
    crate::macos_header_menu_overlay::overlay_close_all(reason);
    crate::macos_header_menu_overlay::clear_btn_open(menus);
    crate::macos_header_menu_debug::log_popdown(reason, menus);
    for btn in menus {
        if let Some(pop) = btn.popover() {
            pop.popdown();
        }
        btn.set_active(false);
    }
    disarm_compositing_hold();
}

