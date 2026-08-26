type MouseMon = RefCell<Option<objc2::rc::Retained<objc2::runtime::AnyObject>>>;

#[derive(Clone)]
struct UnfocusedCursor {
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    cur: Rc<RefCell<Option<glib::SourceId>>>,
    ptr: Rc<Cell<bool>>,
    sq: Rc<Cell<Option<Instant>>>,
    lgl: Rc<Cell<Option<(f64, f64)>>>,
}

fn drop_mouse_monitor(slot: &MouseMon) {
    if let Some(mon) = slot.borrow_mut().take() {
        unsafe {
            objc2_app_kit::NSEvent::removeMonitor(&mon);
        }
    }
}

impl UnfocusedCursor {
    fn leave(&self) {
        self.ptr.set(false);
        self.lgl.set(None);
        drop_glib_source(self.cur.as_ref());
        show_chrome_pointer(&self.win, &self.gl);
    }

    fn arm_hide_timer(&self) {
        let s = self.clone();
        replace_timeout(Rc::clone(&self.cur), move || {
            if !s.ptr.get() {
                return;
            }
            if !pointer_over_video_gl(&s.win, &s.gl) {
                s.leave();
                return;
            }
            if !apply_theater_cursor_hide(&s.win, &s.gl, &s.player) {
                s.leave();
            }
        });
    }

    fn theater_ready(&self) -> bool {
        self.gl.is_mapped()
            && self.gl.is_visible()
            && !self.recent.is_visible()
            && chrome_should_hide_cursor_for_media(&self.player)
    }

    fn hide_now_if_over_video(&self) {
        if !self.theater_ready() || !pointer_over_video_gl(&self.win, &self.gl) {
            self.leave();
            return;
        }
        self.ptr.set(true);
        drop_glib_source(self.cur.as_ref());
        if !apply_theater_cursor_hide(&self.win, &self.gl, &self.player) {
            self.leave();
        }
    }

    fn tick(&self) {
        if self.win.is_active() {
            return;
        }
        if !self.theater_ready() {
            self.leave();
            return;
        }
        let Some((x, y)) = pointer_pick_xy_for_win(&self.win) else {
            self.leave();
            return;
        };
        if !pointer_over_video_gl(&self.win, &self.gl) {
            self.leave();
            return;
        }
        self.tick_advance(x, y);
    }

    /// Pointer is confirmed over live theater video: show it, then rearm the hide timer unless
    /// this sample is squelched / a duplicate position.
    fn tick_advance(&self, x: f64, y: f64) {
        self.ptr.set(true);
        if motion_sample_stale(&self.sq, &self.lgl, x, y) {
            return;
        }
        self.lgl.set(Some((x, y)));
        show_chrome_pointer(&self.win, &self.gl);
        self.arm_hide_timer();
    }
}

fn watch_window_occlusion(win: adw::ApplicationWindow, cursor: UnfocusedCursor) {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSWindowDidChangeOcclusionStateNotification;
    use objc2_foundation::NSNotificationCenter;

    let _ = glib::idle_add_local_once(move || {
        let Some(nswin) = crate::macos_window::nswindow_for_widget(&win) else {
            eprintln!("[rhino] cursor: occlusion watch skipped, no NSWindow");
            return;
        };
        let Some(_mtm) = MainThreadMarker::new() else {
            return;
        };
        let block = RcBlock::new(move |_| {
            if cursor.win.is_active() {
                return;
            }
            cursor.hide_now_if_over_video();
        });
        let center = NSNotificationCenter::defaultCenter();
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidChangeOcclusionStateNotification),
                Some(&nswin),
                None,
                &block,
            )
        };
        std::mem::forget(observer);
    });
}

fn start_unfocused_mouse_monitor(monitor: &Rc<MouseMon>, tick: &Rc<dyn Fn()>) {
    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask};
    use std::ptr::NonNull;

    if monitor.borrow().is_some() {
        return;
    }
    let tick2 = Rc::clone(tick);
    let block = RcBlock::new(move |_: NonNull<NSEvent>| {
        tick2();
    });
    *monitor.borrow_mut() = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
        NSEventMask::MouseMoved | NSEventMask::LeftMouseDragged,
        &block,
    );
    if monitor.borrow().is_none() {
        eprintln!("[rhino] cursor: global mouse monitor was not installed");
    }
}
