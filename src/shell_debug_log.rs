//! Toolbar / bottom-bar layout diagnostics (`RHINO_SHELL_DEBUG=1`).

use glib::object::IsA;
use gtk::prelude::WidgetExt;
use std::sync::OnceLock;

pub(crate) fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("RHINO_SHELL_DEBUG").is_some())
}

pub(crate) fn log(msg: impl std::fmt::Display) {
    if enabled() {
        eprintln!("[rhino] shell: {msg}");
    }
}

fn widget_line(name: &str, w: &impl IsA<gtk::Widget>, root: &impl IsA<gtk::Widget>) -> String {
    let y = w
        .compute_point(root, &gtk::graphene::Point::new(0.0, 0.0))
        .map(|p| p.y());
    let css = w
        .css_classes()
        .into_iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let y_s = y.map(|v| format!("{v:.0}")).unwrap_or_else(|| "?".into());
    format!(
        "{name}={}x{} y={y_s} map={} vis={} opa={:.2} css=[{css}]",
        w.width(),
        w.height(),
        w.is_mapped(),
        w.is_visible(),
        w.opacity(),
    )
}

#[cfg(target_os = "macos")]
fn ns_client_size(win: &adw::ApplicationWindow) -> Option<(i32, i32)> {
    use objc2::msg_send;
    use objc2_app_kit::NSView;
    use objc2_foundation::NSRect;

    let nsw = crate::macos_window::nswindow_for_widget(win)?;
    unsafe {
        let cv: *mut NSView = msg_send![&*nsw, contentView];
        if cv.is_null() {
            return None;
        }
        let frame: NSRect = msg_send![cv, frame];
        Some((frame.size.width as i32, frame.size.height as i32))
    }
}

#[cfg(not(target_os = "macos"))]
fn ns_client_size(_win: &adw::ApplicationWindow) -> Option<(i32, i32)> {
    None
}

pub(crate) fn log_toolbar_layout(
    tag: &str,
    win: &adw::ApplicationWindow,
    root: &adw::ToolbarView,
    header: &adw::HeaderBar,
    bottom: &gtk::Box,
    gl: &gtk::GLArea,
    recent_vis: bool,
    bar_show: bool,
    show: bool,
    #[cfg(target_os = "macos")] bottom_shell: &gtk::Box,
) {
    if !enabled() {
        return;
    }
    let mut msg = format!(
        "{tag} show={show} bar_show={bar_show} recent={recent_vis} \
         reveal_top={} reveal_bottom={} top_h={} bottom_h={}",
        root.reveals_top_bars(),
        root.reveals_bottom_bars(),
        root.top_bar_height(),
        root.bottom_bar_height(),
    );
    msg.push_str(" | ");
    msg.push_str(&widget_line("win", win, win));
    msg.push_str(" | ");
    msg.push_str(&widget_line("root", root, win));
    msg.push_str(" | ");
    msg.push_str(&widget_line("hdr", header, win));
    msg.push_str(" | ");
    msg.push_str(&widget_line("gl", gl, win));
    msg.push_str(" | ");
    msg.push_str(&widget_line("bottom", bottom, win));
    #[cfg(target_os = "macos")]
    {
        msg.push_str(" | ");
        msg.push_str(&widget_line("shell", bottom_shell, win));
    }
    if let Some((nw, nh)) = ns_client_size(win) {
        msg.push_str(&format!(
            " | ns={nw}x{nh} gtkΔ={}x{}",
            win.width() - nw,
            win.height() - nh
        ));
    }
    log(msg);
}

pub(crate) fn log_fit(target_w: i32, target_h: i32, win: &adw::ApplicationWindow, video: (i64, i64)) {
    if !enabled() {
        return;
    }
    log(format!(
        "fit video={}x{} target={target_w}x{target_h} gtk={}x{}",
        video.0,
        video.1,
        win.width(),
        win.height()
    ));
}

#[cfg(target_os = "macos")]
pub(crate) fn log_resize_pass(attempt: u8, target_w: i32, target_h: i32, win: &adw::ApplicationWindow, forced: bool) {
    if !enabled() {
        return;
    }
    let ns = ns_client_size(win)
        .map(|(w, h)| format!("{w}x{h}"))
        .unwrap_or_else(|| "?".into());
    log(format!(
        "resize pass={attempt} target={target_w}x{target_h} gtk={}x{} ns={ns} forced={forced}",
        win.width(),
        win.height()
    ));
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn log_resize_pass(_attempt: u8, _target_w: i32, _target_h: i32, _win: &adw::ApplicationWindow, _forced: bool) {}
