// macOS: opaque CSS for the seek preview frame over the native video layer.
// Theater open/close uses `macos_shell_compositing::preview_opened` / `overlay_closed`.

use gtk::prelude::*;

use super::SeekPreviewState;

fn preview_provider() -> &'static gtk::CssProvider {
    Box::leak(Box::new({
        let p = gtk::CssProvider::new();
        p.load_from_string(
            "frame.rp-seek-thumb-frame,\
            frame.rp-seek-thumb-frame > border {\
                background-color: #2d2d2d;\
                background: #2d2d2d;\
            }\
            frame.rp-seek-thumb-frame glarea {\
                background-color: #000000;\
                background: #000000;\
            }",
        );
        p
    }))
}

/// Widget-level opaque paint (display CSS is not enough over the native video layer).
pub(super) fn wire_opaque_frame(st: &SeekPreviewState) {
    st.container.set_opacity(1.0);
    #[allow(deprecated)]
    gtk::prelude::StyleContextExt::add_provider(
        &st.container.style_context(),
        preview_provider(),
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
}

fn win_fullscreen(st: &SeekPreviewState) -> bool {
    st.ovl
        .root()
        .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
        .is_some_and(|w| {
            // AppKit mask is authoritative; GDK can lag or stick after exit.
            crate::macos_window::ns_fullscreen_for_win(&w) || w.is_fullscreen()
        })
}

pub(super) fn on_open(st: &SeekPreviewState) {
    if !win_fullscreen(st) {
        return;
    }
    crate::macos_shell_compositing::preview_opened();
}

/// Every hide refreshes shell chrome — the stale-arrangement bug also occurs windowed.
pub(super) fn on_close() {
    crate::macos_shell_compositing::overlay_closed();
}
