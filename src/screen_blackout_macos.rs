fn wire_screen_params_macos(sync: Rc<BlackoutSync>, btn: gtk::Button) {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplicationDidChangeScreenParametersNotification;
    use objc2_foundation::NSNotificationCenter;

    let Some(_mtm) = MainThreadMarker::new() else {
        return;
    };
    let block = RcBlock::new(move |_notif| {
        sync_btn_visible(&btn);
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

fn screen_count_macos() -> usize {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;

    let Some(mtm) = MainThreadMarker::new() else {
        return 1;
    };
    NSScreen::screens(mtm).len()
}

fn sync_macos(
    bo: &mut ScreenBlackout,
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_visible: bool,
) {
    if !should_apply(bo, win, player, recent_visible) {
        clear_macos(&mut bo.windows);
        bo.video_screen_ptr = None;
        return;
    }
    let Some(main_nswin) = crate::macos_window::nswindow_for_widget(win) else {
        clear_macos(&mut bo.windows);
        bo.video_screen_ptr = None;
        return;
    };
    let Some(video_screen) = main_nswin.screen() else {
        clear_macos(&mut bo.windows);
        bo.video_screen_ptr = None;
        return;
    };
    let video_ptr = objc2::rc::Retained::as_ptr(&video_screen);
    if bo.video_screen_ptr == Some(video_ptr) && !bo.windows.is_empty() {
        return;
    }
    bo.video_screen_ptr = Some(video_ptr);
    apply_macos(&mut bo.windows, &main_nswin, &video_screen);
}

fn apply_macos(
    slots: &mut Vec<objc2::rc::Retained<objc2_app_kit::NSWindow>>,
    main_nswin: &objc2_app_kit::NSWindow,
    video_screen: &objc2::rc::Retained<objc2_app_kit::NSScreen>,
) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSBackingStoreType, NSColor, NSMainMenuWindowLevel, NSScreen, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::NSRect;

    clear_macos(slots);
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let level = NSMainMenuWindowLevel + 1;
    let video_ptr = objc2::rc::Retained::as_ptr(video_screen);
    for screen in NSScreen::screens(mtm).iter() {
        if objc2::rc::Retained::as_ptr(&screen) == video_ptr {
            continue;
        }
        let mut frame: NSRect = screen.frame();
        frame.origin.x = 0.0;
        frame.origin.y = 0.0;
        let black = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer_screen(
                mtm.alloc::<NSWindow>(),
                frame,
                NSWindowStyleMask(0),
                NSBackingStoreType::Buffered,
                false,
                Some(screen.as_ref()),
            )
        };
        black.setBackgroundColor(Some(&NSColor::blackColor()));
        black.setLevel(level);
        black.setIgnoresMouseEvents(true);
        black.orderFront(Some(main_nswin.as_ref()));
        slots.push(black);
    }
}

fn clear_macos(slots: &mut Vec<objc2::rc::Retained<objc2_app_kit::NSWindow>>) {
    for w in slots.drain(..) {
        w.orderOut(None);
    }
}
