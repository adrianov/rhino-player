//! macOS fullscreen-exit tracing (`RHINO_MACOS_FS_DEBUG=1` on stderr).

#[cfg(target_os = "macos")]
use gtk::prelude::{Cast, GtkWindowExt};
#[cfg(target_os = "macos")]
use std::sync::OnceLock;

#[cfg(target_os = "macos")]
fn fs_debug_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("RHINO_MACOS_FS_DEBUG")
            .ok()
            .is_some_and(|v| v != "0" && !v.is_empty())
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn enabled() -> bool {
    fs_debug_enabled()
}

#[cfg(target_os = "macos")]
pub(crate) fn log(msg: &str) {
    if enabled() {
        eprintln!("[rhino] macos-fs: {msg}");
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn log_win_state(tag: &str, win: &adw::ApplicationWindow) {
    if !enabled() {
        return;
    }
    let gtk = win.is_fullscreen();
    let ns = crate::macos_window::nswindow_for_widget(win.upcast_ref::<gtk::Widget>())
        .is_some_and(|w| crate::macos_window::ns_window_is_native_fullscreen(&w));
    let armed = crate::macos_fs_exit::exit_armed();
    eprintln!("[rhino] macos-fs: {tag} gtk_fullscreen={gtk} ns_fullscreen={ns} exit_armed={armed}");
}

/// Temporary diagnostic: one line per contentView sublayer (depth-capped) so a stuck
/// AppKit snapshot / stale chrome backing shows up as an unexpected layer with an odd
/// frame or contentsScale. Used to pin the mid-screen header band mechanism.
#[cfg(target_os = "macos")]
pub(crate) fn dump_window_layers(win: &adw::ApplicationWindow, tag: &str) {
    if !enabled() {
        return;
    }
    let Some(nswin) = crate::macos_window::nswindow_for_widget(win) else {
        return;
    };
    unsafe {
        let cv: *mut objc2_app_kit::NSView = objc2::msg_send![&*nswin, contentView];
        let Some(content_view) = objc2::rc::Retained::retain(cv) else {
            return;
        };
        let Some(root) = content_view.layer() else {
            return;
        };
        walk_layer(&root, 0, tag);
    }
}

#[cfg(target_os = "macos")]
fn walk_layer(layer: &objc2_quartz_core::CALayer, depth: usize, tag: &str) {
    if depth > 5 {
        return;
    }
    let sub_count = unsafe { layer.sublayers() }
        .as_ref()
        .map(|a| a.count())
        .unwrap_or(0);
    eprintln!("{}", format_layer_line(layer, depth, tag, sub_count));
    if let Some(arr) = unsafe { layer.sublayers() } {
        for i in 0..arr.count().min(12) {
            walk_layer(&arr.objectAtIndex(i), depth + 1, tag);
        }
    }
}

#[cfg(target_os = "macos")]
fn format_layer_line(
    layer: &objc2_quartz_core::CALayer,
    depth: usize,
    tag: &str,
    subs: usize,
) -> String {
    let f = layer.frame();
    let b = layer.bounds();
    let cr = layer.contentsRect();
    let cls = layer.class().name().to_string_lossy().into_owned();
    format!(
        "[rhino] macos-fs: layer at={tag} d={depth} {cls} frame={:.0},{:.0} {:.0}x{:.0} bounds={:.0}x{:.0} scale={} crect={:.1},{:.1} {:.1}x{:.1} hidden={} subs={subs}",
        f.origin.x,
        f.origin.y,
        f.size.width,
        f.size.height,
        b.size.width,
        b.size.height,
        layer.contentsScale(),
        cr.origin.x,
        cr.origin.y,
        cr.size.width,
        cr.size.height,
        layer.isHidden(),
    )
}
