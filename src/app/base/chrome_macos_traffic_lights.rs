// Windowed macOS: hide native traffic lights when ToolbarView bars begin hiding; show again once
// top_bar_height / bottom_bar_height match on two consecutive polls after reveal (GtkRevealer slide).
// While the window is fullscreen, lights stay visible — hiding them during AppKit fullscreen
// transitions regressed on macOS 26.

#[cfg(target_os = "macos")]
thread_local! {
    static MACOS_TRAFFIC_POLL: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
    static MACOS_TRAFFIC_HEIGHT_SNAP: RefCell<Option<(i32, i32)>> = const { RefCell::new(None) };
}

#[cfg(target_os = "macos")]
fn macos_traffic_cancel_poll() {
    MACOS_TRAFFIC_POLL.with(|s| {
        drop_glib_source(s);
    });
    MACOS_TRAFFIC_HEIGHT_SNAP.with(|h| *h.borrow_mut() = None);
}

#[cfg(target_os = "macos")]
fn macos_traffic_poll_until_stable(hdr: adw::HeaderBar, root: adw::ToolbarView) {
    macos_traffic_cancel_poll();
    let started = Instant::now();
    let id = glib::timeout_add_local(Duration::from_millis(16), move || {
        if started.elapsed() > Duration::from_millis(900) {
            crate::macos_window::set_traffic_lights_visible(&hdr, true);
            macos_traffic_cancel_poll();
            return glib::ControlFlow::Break;
        }
        if !root.reveals_top_bars() || !root.reveals_bottom_bars() {
            return glib::ControlFlow::Continue;
        }
        let th = root.top_bar_height();
        let bh = root.bottom_bar_height();
        let settled = MACOS_TRAFFIC_HEIGHT_SNAP.with(|hcell| {
            let mut slot = hcell.borrow_mut();
            let prev = slot.take();
            *slot = Some((th, bh));
            prev.is_some_and(|p| p == (th, bh) && th > 0 && bh > 0)
        });
        if settled {
            crate::macos_window::set_traffic_lights_visible(&hdr, true);
            macos_traffic_cancel_poll();
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
    MACOS_TRAFFIC_POLL.with(|s| *s.borrow_mut() = Some(id));
}
