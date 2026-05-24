// macOS gdk-macos: seek preview overlay can leave stale header tiles on the video layer in theater mode.

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
                opacity: 1;\
            }\
            frame.rp-seek-thumb-frame glarea {\
                background-color: #000000;\
                background: #000000;\
                opacity: 1;\
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

fn wire_opaque_preview(st: &SeekPreviewState) {
    attach_provider(st.container.upcast_ref());
    crate::macos_header_menu::attach_opaque_widget(st.container.upcast_ref());
}

fn win_fullscreen(st: &SeekPreviewState) -> bool {
    st.ovl
        .root()
        .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
        .is_some_and(|w| w.is_fullscreen())
}

pub(super) fn on_open(st: &SeekPreviewState) {
    if !win_fullscreen(st) {
        return;
    }
    wire_opaque_preview(st);
    crate::macos_header_menu_overlay::raise_overlay_child(&st.ovl, &st.container);
    st.container.queue_allocate();
    st.gl.queue_render();
    crate::macos_header_menu::on_overlay_surface_opened();
}

pub(super) fn on_close() {
    crate::macos_header_menu::on_menu_surface_closed();
}
