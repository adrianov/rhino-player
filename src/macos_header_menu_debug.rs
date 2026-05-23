//! macOS header [`gtk::MenuButton`] / [`gtk::Popover`] tracing (temporary; remove when stable).

use gtk::prelude::*;

fn pop_vis(btn: &gtk::MenuButton) -> bool {
    btn.popover().is_some_and(|p| p.is_visible())
}

fn menu_state(btn: &gtk::MenuButton) -> String {
    format!("active={} pop_visible={}", btn.is_active(), pop_vis(btn))
}

pub(crate) fn log_event(menu: &str, event: &str, detail: &str) {
    let bt = std::backtrace::Backtrace::capture();
    if detail.is_empty() {
        eprintln!("[rhino] macos-menu: {event} menu={menu}");
    } else {
        eprintln!("[rhino] macos-menu: {event} menu={menu} {detail}");
    }
    eprintln!("{bt}");
}

pub(crate) fn log_popdown(reason: &str, menus: &[gtk::MenuButton]) {
    let states: Vec<String> = menus
        .iter()
        .map(|b| format!("active={} pop={}", b.is_active(), pop_vis(b)))
        .collect();
    log_event(
        "*",
        "close",
        &format!("reason={reason} menus=[{}]", states.join(", ")),
    );
}

/// Show / hide / active transitions for one header menu control.
pub(crate) fn wire_header_menu_trace(name: &'static str, btn: &gtk::MenuButton, pop: &gtk::Popover) {
    let btn_act = btn.clone();
    btn.connect_active_notify(move |b| {
        log_event(name, "active_notify", &menu_state(b));
    });
    let pop_show = pop.clone();
    pop_show.connect_show(move |p| {
        log_event(
            name,
            "open",
            &format!("popover_show visible={}", p.is_visible()),
        );
    });
    let pop_map = pop.clone();
    pop_map.connect_map(move |p| {
        log_event(name, "open", &format!("popover_map visible={}", p.is_visible()));
    });
    let btn_cls = btn_act.clone();
    pop.connect_closed(move |_| {
        log_event(name, "close", &format!("popover_closed {}", menu_state(&btn_cls)));
    });
}
