fn wire_screen_params_macos(sync: Rc<BlackoutSync>) {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplicationDidChangeScreenParametersNotification;
    use objc2_foundation::NSNotificationCenter;

    let Some(_mtm) = MainThreadMarker::new() else {
        return;
    };
    // `queue: None` → the block runs on the posting thread, which is main for this notification,
    // so the GTK-side sync needs no hop. `NSOperationQueue::mainQueue` would deliver through
    // libdispatch instead, outside the GLib main context.
    let block = RcBlock::new(move |_notif| {
        sync.sync();
    });
    let center = NSNotificationCenter::defaultCenter();
    let _observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSApplicationDidChangeScreenParametersNotification),
            None,
            None,
            &block,
        )
    };
    std::mem::forget(_observer);
}

fn wire_nswin_screen_macos(sync: Rc<BlackoutSync>) {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSWindowDidChangeScreenNotification;
    use objc2_foundation::NSNotificationCenter;

    let win = sync.win.clone();
    let _ = glib::idle_add_local_once(move || {
        let Some(nswin) = crate::macos_window::nswindow_for_widget(&win) else {
            return;
        };
        let Some(_mtm) = MainThreadMarker::new() else {
            return;
        };
        let block = RcBlock::new(move |_notif| {
            sync.sync();
        });
        let center = NSNotificationCenter::defaultCenter();
        let _observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidChangeScreenNotification),
                Some(&nswin),
                None,
                &block,
            )
        };
        std::mem::forget(_observer);
    });
}
