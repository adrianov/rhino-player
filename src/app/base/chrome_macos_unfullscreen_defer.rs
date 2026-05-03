#[cfg(target_os = "macos")]
thread_local! {
    static MACOS_DEFER_UNFULLSCREEN: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
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
                if !win.is_fullscreen() {
                    return;
                }
                win.unfullscreen();
            },
        );
        *slot.borrow_mut() = Some(id);
    });
}
