// Re-snapshot chrome when a window regains focus (stale gdk-macos layers vs video).
fn wire_focus_return_repaint(
    ctx: &WindowInputCtx,
    touch_chrome_gl: Rc<dyn Fn(&adw::ApplicationWindow)>,
) {
    let root_ia = ctx.shell.root.clone();
    let vh_ia = ctx.shell.video_handle.clone();
    let win_focus = ctx.shell.win.clone();
    let tch = touch_chrome_gl;
    win_focus.connect_is_active_notify(move |w| {
        if !w.is_active() {
            return;
        }
        // Drop AppKit's cached layer snapshot in both windowed and fullscreen modes —
        // otherwise the cross-fade on foreground return leaves a ghosted header band.
        #[cfg(target_os = "macos")]
        crate::macos_window::invalidate_window_layers(w);
        if !w.is_fullscreen() {
            return;
        }
        tch(w);
        if let Some(surf) = w.native().and_then(|n| n.surface()) {
            surf.queue_render();
        }
        root_ia.queue_allocate();
        vh_ia.queue_draw();
        let tch2 = Rc::clone(&tch);
        let w2 = w.clone();
        let _ = glib::source::idle_add_local_once(move || {
            tch2(&w2);
            #[cfg(target_os = "macos")]
            crate::macos_window::invalidate_window_layers(&w2);
        });
    });
}
