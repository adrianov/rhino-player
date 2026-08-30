// macOS: seek preview uses its own popup surface, independent of window chrome.

use gtk::prelude::*;

use super::{SeekPreviewState, PREVIEW_GAP};

fn preview_provider() -> &'static gtk::CssProvider {
    Box::leak(Box::new({
        let p = gtk::CssProvider::new();
        p.load_from_string(
            "frame.rp-seek-thumb-frame {\
                background-color: #2d2d2d;\
                background: #2d2d2d;\
            }\
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

pub(super) fn build_popup(seek: &gtk::Scale, frame: &gtk::Frame) -> gtk::Popover {
    let popup = gtk::Popover::new();
    configure_popup(&popup);
    popup.set_parent(seek);
    popup.set_child(Some(frame));
    popup.add_css_class("rp-seek-popover");
    attach_provider(popup.upcast_ref());
    popup
}

/// Keep the preview noninteractive so showing it cannot focus or activate the window.
fn configure_popup(popup: &gtk::Popover) {
    popup.set_autohide(false);
    popup.set_can_focus(false);
    popup.set_can_target(false);
    popup.set_has_arrow(false);
    popup.set_position(gtk::PositionType::Top);
    popup.set_offset(0, -PREVIEW_GAP);
    if popup.find_property("modal").is_some() {
        popup.set_property("modal", false);
    }
}

fn attach_provider(widget: &gtk::Widget) {
    #[allow(deprecated)]
    gtk::prelude::StyleContextExt::add_provider(
        &widget.style_context(),
        preview_provider(),
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
}

/// Widget-level opaque paint on the independent popup surface.
pub(super) fn wire_opaque_frame(st: &SeekPreviewState) {
    st.container.set_opacity(1.0);
    attach_provider(st.container.upcast_ref());
}

pub(super) fn point_at(st: &SeekPreviewState, x: f64) {
    let width = f64::from(st.seek.width().max(1));
    let x = x.clamp(2.0, (width - 2.0).max(2.0)) as i32;
    st.popup
        .set_pointing_to(Some(&gtk::gdk::Rectangle::new(x, -PREVIEW_GAP, 1, 1)));
}
