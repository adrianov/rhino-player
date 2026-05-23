// Programmatic resize + stale gdk-macos compositing refresh for the GTK shell.

/// Ask gdk-macos to relayout the toplevel during the next surface layout pass.
pub fn request_gdk_surface_layout<W: IsA<gtk::Widget>>(widget: &W) {
    use gtk::gdk::prelude::SurfaceExt;
    use gtk::prelude::{NativeExt, WidgetExt};

    widget.queue_resize();
    if let Some(surf) = widget.native().and_then(|n| n.surface()) {
        surf.request_layout();
    }
}

fn force_nswindow_frame(win: &adw::ApplicationWindow, width: i32, height: i32) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSWindowAnimationBehavior;
    use objc2_foundation::NSSize;

    let Some(nswin) = nswindow_for_widget(win) else {
        return;
    };
    if MainThreadMarker::new().is_none() {
        return;
    }
    let w = f64::from(width.clamp(320, 4096));
    let h = f64::from(height.clamp(200, 4096));
    nswin.setAnimationBehavior(NSWindowAnimationBehavior::None);
    let mut frame = nswin.frame();
    frame.size = NSSize::new(w, h);
    nswin.setFrame_display(frame, true);
}

/// Apply `(width, height)` to a realized window. Prefer the GDK layout path so ToolbarView
/// chrome and the native video layer stay synchronized; direct [`NSWindow::setFrame`] alone
/// leaves gdk-macos compositing at stale geometry (VOB / DVD fit-on-open vs default 960×540).
pub fn resize_window_frame(win: &adw::ApplicationWindow, width: i32, height: i32) {
    use gtk::prelude::{GtkWindowExt, WidgetExt};

    let w = width.clamp(320, 4096);
    let h = height.clamp(200, 4096);
    win.set_default_size(w, h);
    for attempt in 0..3 {
        request_gdk_surface_layout(win);
        win.queue_allocate();
        let forced = attempt > 0;
        crate::shell_debug_log::log_resize_pass(attempt, w, h, win, forced);
        if win.width() == w && win.height() == h {
            invalidate_window_layers(win);
            crate::shell_debug_log::log_resize_pass(9, w, h, win, forced);
            crate::app::schedule_shell_layout_after_gtk_resize(w, h);
            return;
        }
        if forced {
            force_nswindow_frame(win, w, h);
            win.set_default_size(w, h);
        }
    }
    request_gdk_surface_layout(win);
    win.queue_allocate();
    invalidate_window_layers(win);
    crate::shell_debug_log::log_resize_pass(8, w, h, win, true);
    let win2 = win.clone();
    let w2 = w;
    let h2 = h;
    let _ = glib::idle_add_local_once(move || {
        request_gdk_surface_layout(&win2);
        win2.queue_allocate();
        invalidate_window_layers(&win2);
        crate::shell_debug_log::log_resize_pass(7, w2, h2, &win2, true);
        crate::app::schedule_shell_layout_after_gtk_resize(w2, h2);
    });
}

/// Drop stale gdk-macos compositing after geometry changes (ghost header / continue grid).
pub fn refresh_gdk_shell_compositing(
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    header: &adw::HeaderBar,
    root: &adw::ToolbarView,
    bottom_shell: &gtk::Box,
    bottom: &gtk::Box,
) {
    use gtk::gdk::prelude::SurfaceExt;
    use gtk::prelude::{NativeExt, WidgetExt};

    crate::macos_bottom_bar::repaint_opaque(bottom_shell, bottom);
    header.queue_draw();
    root.queue_draw();
    gl.queue_draw();
    win.queue_draw();
    request_gdk_surface_layout(win);
    if let Some(surf) = win.native().and_then(|n| n.surface()) {
        surf.queue_render();
    }
    invalidate_window_layers(win);
    let win2 = win.clone();
    let gl2 = gl.clone();
    let header2 = header.clone();
    let root2 = root.clone();
    let shell2 = bottom_shell.clone();
    let bottom2 = bottom.clone();
    let _ = glib::idle_add_local_once(move || {
        crate::macos_bottom_bar::repaint_opaque(&shell2, &bottom2);
        header2.queue_draw();
        root2.queue_draw();
        gl2.queue_draw();
        win2.queue_draw();
        invalidate_window_layers(&win2);
    });
}
