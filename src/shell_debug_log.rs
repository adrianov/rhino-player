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

fn push_widget_line(
    msg: &mut String,
    name: &str,
    w: &impl IsA<gtk::Widget>,
    root: &impl IsA<gtk::Widget>,
) {
    msg.push_str(" | ");
    msg.push_str(&widget_line(name, w, root));
}

/// Widget handles logged by [`log_toolbar_layout`].
pub(crate) struct ToolbarLayoutRefs<'a> {
    pub(crate) win: &'a adw::ApplicationWindow,
    pub(crate) root: &'a adw::ToolbarView,
    pub(crate) header: &'a adw::HeaderBar,
    pub(crate) bottom: &'a gtk::Box,
    pub(crate) gl: &'a gtk::GLArea,
    #[cfg(target_os = "macos")]
    pub(crate) bottom_shell: &'a gtk::Box,
}

pub(crate) fn log_toolbar_layout(
    tag: &str,
    w: &ToolbarLayoutRefs<'_>,
    recent_vis: bool,
    bar_show: bool,
    show: bool,
) {
    if !enabled() {
        return;
    }
    let mut msg = format!(
        "{tag} show={show} bar_show={bar_show} recent={recent_vis} \
         reveal_top={} reveal_bottom={} top_h={} bottom_h={}",
        w.root.reveals_top_bars(),
        w.root.reveals_bottom_bars(),
        w.root.top_bar_height(),
        w.root.bottom_bar_height(),
    );
    push_widget_line(&mut msg, "win", w.win, w.win);
    push_widget_line(&mut msg, "root", w.root, w.win);
    push_widget_line(&mut msg, "hdr", w.header, w.win);
    push_widget_line(&mut msg, "gl", w.gl, w.win);
    push_widget_line(&mut msg, "bottom", w.bottom, w.win);
    #[cfg(target_os = "macos")]
    push_widget_line(&mut msg, "shell", w.bottom_shell, w.win);
    if let Some((nw, nh)) = ns_client_size(w.win) {
        msg.push_str(&format!(
            " | ns={nw}x{nh} gtkΔ={}x{}",
            w.win.width() - nw,
            w.win.height() - nh
        ));
    }
    log(msg);
}

pub(crate) fn log_fit(
    target_w: i32,
    target_h: i32,
    win: &adw::ApplicationWindow,
    video: (i64, i64),
) {
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
pub(crate) fn log_resize_pass(
    attempt: u8,
    target_w: i32,
    target_h: i32,
    win: &adw::ApplicationWindow,
    forced: bool,
) {
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
