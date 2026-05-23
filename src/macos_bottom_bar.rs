//! macOS gdk-macos: bottom chrome uses the same `.rpb-header` surface as the top bar.
//! Display-level CSS does not composite opaque over the native video layer; a widget-level
//! provider is required (transparent window + `CAOpenGLLayer` under gdk-macos GTK tiles).

use glib::object::Cast;
use gtk::prelude::{BoxExt, WidgetExt};

fn chrome_provider() -> &'static gtk::CssProvider {
    Box::leak(Box::new({
        let p = gtk::CssProvider::new();
        p.load_from_string(
            "box.rp-bottom-shell {\
                background-color: #2d2d2d;\
                background: #2d2d2d;\
                opacity: 1;\
            }\
            box.rp-bottom-shell box.rp-bottom {\
                background-color: transparent;\
                background: transparent;\
            }",
        );
        p
    }))
}

fn wire_paint(shell: &gtk::Box, row: &gtk::Box) {
    let provider = chrome_provider();
    let shell_w: &gtk::Widget = shell.upcast_ref();
    let row_w: &gtk::Widget = row.upcast_ref();
    for w in [shell_w, row_w] {
        #[allow(deprecated)]
        gtk::prelude::StyleContextExt::add_provider(
            &w.style_context(),
            provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );
    }
}

/// Opaque shell for [`adw::ToolbarView::add_bottom_bar`] on macOS.
pub fn wrap_row(row: &gtk::Box) -> gtk::Box {
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shell.add_css_class("rpb-header");
    shell.add_css_class("rp-bottom-shell");
    shell.set_vexpand(false);
    shell.set_opacity(1.0);
    shell.append(row);
    wire_paint(&shell, row);
    shell
}

pub fn repaint_opaque(shell: &gtk::Box, row: &gtk::Box) {
    shell.queue_draw();
    row.queue_draw();
}
