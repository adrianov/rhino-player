impl MpvBundle {
    /// Subscribe to mpv property changes. Each tuple is `(reply_id, name, format)`.
    /// `reply_id` is echoed back on the [Event::PropertyChange] so handlers can dispatch quickly.
    pub fn observe_props(&self, props: &[(u64, &str, Format)]) -> Result<(), String> {
        for (id, name, fmt) in props {
            self.mpv
                .observe_property(name, *fmt, *id)
                .map_err(|e| format!("observe_property {name}: {e:?}"))?;
        }
        Ok(())
    }

    /// Wakeup-driven mpv event drain. The closure runs **on the GTK main thread** whenever
    /// libmpv has new events; the caller drains them with [drain_events]. The mpv wakeup
    /// callback is invoked from arbitrary mpv threads, so the closure is parked in a
    /// thread-local registered on the main thread, and a `Send` shim hops back over
    /// `MainContext::invoke`. See `events-over-polling.mdc`: do not call other mpv APIs
    /// from the wakeup callback itself.
    pub fn install_event_drain<F: Fn() + 'static>(&mut self, on_main: F) {
        thread_local! {
            static DRAIN: std::cell::RefCell<Option<Box<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
        }
        fn call_drain() {
            DRAIN.with(|s| { if let Some(f) = s.borrow().as_ref() { f(); } });
        }
        DRAIN.with(|s| *s.borrow_mut() = Some(Box::new(on_main)));
        let mctx = glib::MainContext::default();
        self.mpv.set_wakeup_callback(move || {
            mctx.clone().invoke(call_drain);
        });
    }

    /// macOS only: register a GTK widget whose visibility hides the native video layer.
    /// Call once after [`MpvBundle::new`] with the recent grid (or any overlay that GTK
    /// stacks on top of the GLArea) so closing the video reveals it.
    #[cfg(target_os = "macos")]
    pub fn watch_overlay<W: glib::object::IsA<gtk::Widget>>(&self, widget: &W) {
        if let Some(m) = self.macos.as_ref() {
            m.watch_overlay(widget);
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn watch_overlay<W: glib::object::IsA<gtk::Widget>>(&self, _widget: &W) {}

    /// Continue-grid warm preload / post-resize: resync native layer frame; optional repin after shell resize.
    pub(crate) fn nudge_browse_video_layout(&self, gl: &gtk::GLArea) {
        self.nudge_video_layout(gl, false);
    }

    pub(crate) fn nudge_shell_layout_after_resize(&self, gl: &gtk::GLArea) {
        self.nudge_video_layout(gl, true);
    }

    fn nudge_video_layout(&self, gl: &gtk::GLArea, _repin_gtk_stack: bool) {
        #[cfg(target_os = "macos")]
        if let Some(m) = self.macos.as_ref() {
            m.resync_layer_frame();
            if _repin_gtk_stack {
                m.repin_below_gtk_compositing();
            }
        }
        gl.queue_render();
    }

    /// macOS only: clear the GLArea framebuffer with alpha=0 so the native video layer
    /// below shows through. Call from inside `connect_render`. Reuses the bundle's
    /// existing `OpenGL.framework` handle — no second `dlopen`.
    #[cfg(target_os = "macos")]
    pub fn clear_glarea_transparent(&self) {
        if let Some(m) = self.macos.as_ref() {
            m.clear_glarea_transparent();
        }
    }

    /// Drain libmpv events until the queue is empty, dispatching each to `handler`.
    /// Call from the closure registered by [install_event_drain].
    ///
    /// Returns `true` when libmpv2 surfaces a **load/demux** failure as `wait_event` `Err`
    /// (it maps `EndFile` with a file error that way — see libmpv2 `events.rs`). Property /
    /// command `Err` values are ignored so they do not look like open failures.
    pub fn drain_events(&mut self, mut handler: impl FnMut(Event<'_>)) -> bool {
        let mut load_failed = false;
        while let Some(ev) = self.mpv.wait_event(0.0) {
            match ev {
                Ok(e) => handler(e),
                Err(e) => {
                    if wait_event_err_is_load_fail(&e) {
                        load_failed = true;
                    }
                    if std::env::var_os("RHINO_TRANSPORT_TRACE").is_some() {
                        eprintln!("[rhino] transport wait_event err: {e:?}");
                    }
                }
            }
        }
        load_failed
    }

    /// End embedded playback for process quit without going through [`libmpv2::Mpv`]'s `Drop`, which
    /// invokes `mpv_destroy` and aborted with GTK `vo=libmpv` on macOS (`mp_clients_destroy`).
    ///
    /// Linux: run [`teardown_gl_paint`] with `gl_area` current earlier in the teardown chain;
    /// `dispose_for_quit` calls [`gtk::prelude::GLAreaExt::make_current`] again before freeing
    /// the render context and calling `mpv_terminate_destroy`.
    ///
    /// macOS: GLArea is a sizing placeholder only; the native render surface is freed before
    /// terminating mpv.
    #[cfg(not(target_os = "macos"))]
    pub fn dispose_for_quit(self, gl_area: &gtk::GLArea) {
        gl_area.make_current();
        let Self {
            _gl,
            mut mpv,
            render,
            ..
        } = self;
        mpv.set_wakeup_callback(|| {});
        drop(render);
        unsafe {
            libmpv2_sys::mpv_terminate_destroy(mpv.ctx.as_ptr());
        }
        std::mem::forget(mpv);
    }

    #[cfg(target_os = "macos")]
    pub fn dispose_for_quit(mut self, _gl_area: &gtk::GLArea) {
        // Drop the native render surface first so its dispatch queue stops touching the
        // mpv render context before we tear it down.
        self.macos.take();
        let Self { mut mpv, .. } = self;
        mpv.set_wakeup_callback(|| {});
        unsafe {
            libmpv2_sys::mpv_terminate_destroy(mpv.ctx.as_ptr());
        }
        std::mem::forget(mpv);
    }
}
