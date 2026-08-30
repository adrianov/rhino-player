// macOS: opaque CSS for the seek preview frame over the native video layer.
// Does not call macos_shell_compositing — that flashes chrome on unfocused theater hover.

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
