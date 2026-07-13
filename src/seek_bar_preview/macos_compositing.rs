// macOS: opaque CSS for the seek preview frame over the native video layer.
// Compositing open/close is owned by `macos_shell_compositing` — not duplicated here.

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

fn attach_provider(w: &gtk::Widget) {
    #[allow(deprecated)]
    gtk::prelude::StyleContextExt::add_provider(
        &w.style_context(),
        preview_provider(),
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
}

/// Widget-level opaque paint (display CSS is not enough over the native video layer).
pub(super) fn wire_opaque_frame(st: &SeekPreviewState) {
    st.container.set_opacity(1.0);
    attach_provider(st.container.upcast_ref());
    crate::macos_header_menu::attach_opaque_widget(st.container.upcast_ref());
}

pub(super) fn win_fullscreen(st: &SeekPreviewState) -> bool {
    st.ovl
        .root()
        .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
        .is_some_and(|w| w.is_fullscreen())
}

pub(super) fn on_open(st: &SeekPreviewState) {
    if !win_fullscreen(st) {
        return;
    }
    st.gl.queue_render();
    crate::macos_shell_compositing::overlay_opened();
}

/// Every hide refreshes shell chrome — the stale-arrangement bug also occurs windowed.
pub(super) fn on_close() {
    crate::macos_shell_compositing::overlay_closed();
}
