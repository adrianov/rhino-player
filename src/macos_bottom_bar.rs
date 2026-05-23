//! macOS gdk-macos: bottom chrome uses the same `.rpb-header` surface as the top bar.
//! [`gtk::Frame`] CSS does not composite opaque over the native video layer on gdk-macos.
//! Opaque colors live in [`crate::theme`] (`MACOS_TRANSPARENT_CONTENT_CSS`).

use gtk::prelude::{BoxExt, WidgetExt};

/// Opaque shell for [`adw::ToolbarView::add_bottom_bar`] on macOS.
pub fn wrap_row(row: &gtk::Box) -> gtk::Box {
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shell.add_css_class("rpb-header");
    shell.add_css_class("rp-bottom-shell");
    shell.set_vexpand(false);
    shell.set_opacity(1.0);
    shell.append(row);
    shell
}

pub fn repaint_opaque(shell: &gtk::Box, row: &gtk::Box) {
    shell.queue_draw();
    row.queue_draw();
}
