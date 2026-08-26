include!("screen_blackout_macos_dispatch.rs");

fn build_cover_windows(
    video_screen: &objc2::rc::Retained<objc2_app_kit::NSScreen>,
) -> Vec<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;

    let Some(mtm) = MainThreadMarker::new() else {
        return Vec::new();
    };
    let video_ptr = objc2::rc::Retained::as_ptr(video_screen);
    let mut slots = Vec::new();
    for screen in NSScreen::screens(mtm).iter() {
        if objc2::rc::Retained::as_ptr(&screen) == video_ptr {
            continue;
        }
        slots.push(make_cover_window(mtm, &screen));
    }
    slots
}

/// One borderless black window pinned above the menu level, covering [screen] fully.
fn make_cover_window(
    mtm: objc2::MainThreadMarker,
    screen: &objc2::rc::Retained<objc2_app_kit::NSScreen>,
) -> objc2::rc::Retained<objc2_app_kit::NSWindow> {
    use objc2_app_kit::{
        NSBackingStoreType, NSColor, NSMainMenuWindowLevel, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::NSRect;

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
    black.setLevel(NSMainMenuWindowLevel + 1);
    black.orderFrontRegardless();
    crate::macos_window::attach_blank_cursor_content(&black);
    black
}

fn playback_session_active(player: &Rc<RefCell<Option<MpvBundle>>>, recent_visible: bool) -> bool {
    if recent_visible {
        return false;
    }
    let (has_path, paused) = mpv_media_state(player);
    has_path && (!paused || tech_hold_active())
}

fn should_apply(
    bo: &ScreenBlackout,
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_visible: bool,
) -> bool {
    let screens = screen_count_macos();
    let apply =
        bo.enabled && win.is_active() && screens >= 2 && playback_session_active(player, recent_visible);
    log_cover_decision(apply, bo.enabled, win.is_active(), screens, player);
    apply
}

fn screen_count_macos() -> usize {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;

    let Some(mtm) = MainThreadMarker::new() else {
        return 1;
    };
    NSScreen::screens(mtm).len()
}

fn queue_clear(bo: &mut ScreenBlackout) {
    let old = std::mem::take(&mut bo.windows);
    bo.video_screen_ptr = None;
    bo.last_screen_count = 0;
    bo.cover_pending = false;
    if !old.is_empty() {
        hop_appkit(AppkitOp::Clear(old));
    }
}

fn sync_macos(
    bo_rc: &Rc<RefCell<ScreenBlackout>>,
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_visible: bool,
) {
    let op = {
        let mut bo = bo_rc.borrow_mut();
        if !should_apply(&bo, win, player, recent_visible) {
            queue_clear(&mut bo);
            return;
        }
        match plan_rebuild(bo_rc, &mut bo, win) {
            Some(op) => op,
            None => return,
        }
    };
    hop_appkit(op);
}

/// Queues a cover rebuild when the video screen or display topology changed since the last sync;
/// [None] when the current covers are still valid (after clearing for missing screens).
fn plan_rebuild(
    bo_rc: &Rc<RefCell<ScreenBlackout>>,
    bo: &mut ScreenBlackout,
    win: &adw::ApplicationWindow,
) -> Option<AppkitOp> {
    let Some(main_nswin) = crate::macos_window::nswindow_for_widget(win) else {
        queue_clear(bo);
        return None;
    };
    let Some(video_screen) = main_nswin.screen() else {
        queue_clear(bo);
        return None;
    };
    let screen_count = screen_count_macos();
    let video_ptr = objc2::rc::Retained::as_ptr(&video_screen);
    if bo.video_screen_ptr == Some(video_ptr)
        && bo.last_screen_count == screen_count
        && (!bo.windows.is_empty() || bo.cover_pending)
    {
        return None;
    }
    let old = std::mem::take(&mut bo.windows);
    bo.video_screen_ptr = Some(video_ptr);
    bo.last_screen_count = screen_count;
    bo.cover_pending = true;
    Some(AppkitOp::Rebuild {
        old,
        video: video_screen,
        dest: Rc::clone(bo_rc),
    })
}
