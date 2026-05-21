//! macOS fullscreen-exit tracing (`RHINO_MACOS_FS_DEBUG=1` on stderr).

#[cfg(target_os = "macos")]
use gtk::prelude::{Cast, GtkWindowExt};
#[cfg(target_os = "macos")]
use std::sync::OnceLock;

#[cfg(target_os = "macos")]
pub(crate) fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("RHINO_MACOS_FS_DEBUG")
            .ok()
            .is_some_and(|v| v != "0" && !v.is_empty())
    })
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
    eprintln!(
        "[rhino] macos-fs: {tag} gtk_fullscreen={gtk} ns_fullscreen={ns} exit_armed={armed}"
    );
}
