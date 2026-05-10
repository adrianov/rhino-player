#[cfg(target_os = "macos")]
thread_local! {
    static MACOS_DEFER_UNFULLSCREEN: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}

#[cfg(target_os = "macos")]
fn macos_timer_chain_then_unfullscreen(win: adw::ApplicationWindow) {
    let win2 = win.clone();
    let _ = glib::timeout_add_local_once(std::time::Duration::ZERO, move || {
        let win3 = win2.clone();
        let _ = glib::timeout_add_local_once(std::time::Duration::ZERO, move || {
            let win4 = win3.clone();
            let _ = glib::timeout_add_local_once(std::time::Duration::ZERO, move || {
                if win4.is_fullscreen() {
                    win4.unfullscreen();
                }
            });
        });
    });
}

#[cfg(target_os = "macos")]
fn macos_schedule_unfullscreen(win: adw::ApplicationWindow) {
    MACOS_DEFER_UNFULLSCREEN.with(|slot| {
        if let Some(id) = slot.borrow_mut().take() {
            id.remove();
        }
        let id = glib::timeout_add_local_once(
            crate::fullscreen_timing::TRANSITION_SETTLE,
            move || {
                MACOS_DEFER_UNFULLSCREEN.with(|s| {
                    *s.borrow_mut() = None;
                });
                macos_timer_chain_then_unfullscreen(win);
            },
        );
        *slot.borrow_mut() = Some(id);
    });
}
