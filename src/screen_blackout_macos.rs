/// AppKit-only work; never touch GTK/GObject here (corrupts GLib idle sources).
enum AppkitOp {
    Clear(Vec<objc2::rc::Retained<objc2_app_kit::NSWindow>>),
    Rebuild {
        old: Vec<objc2::rc::Retained<objc2_app_kit::NSWindow>>,
        video: objc2::rc::Retained<objc2_app_kit::NSScreen>,
        dest: Rc<RefCell<ScreenBlackout>>,
    },
}

fn hop_appkit(op: AppkitOp) {
    use std::ffi::c_void;

    extern "C" {
        static _dispatch_main_q: c_void;
        fn dispatch_async_f(
            queue: *const c_void,
            context: *mut c_void,
            work: unsafe extern "C" fn(*mut c_void),
        );
    }

    let raw = Box::into_raw(Box::new(op));
    unsafe {
        dispatch_async_f(
            &_dispatch_main_q as *const c_void,
            raw.cast(),
            blackout_appkit_dispatch,
        );
    }
}

fn order_out_all(windows: Vec<objc2::rc::Retained<objc2_app_kit::NSWindow>>) {
    for w in windows {
        w.orderOut(None);
    }
}

unsafe extern "C" fn blackout_appkit_dispatch(raw: *mut std::ffi::c_void) {
    let op = unsafe { *Box::from_raw(raw.cast::<AppkitOp>()) };
    match op {
        AppkitOp::Clear(windows) => order_out_all(windows),
        AppkitOp::Rebuild { old, video, dest } => {
            order_out_all(old);
            let built = build_cover_windows(&video);
            let mut g = dest.borrow_mut();
            // Drop any windows a prior in-flight rebuild already stored.
            order_out_all(std::mem::take(&mut g.windows));
            g.cover_pending = false;
            if g.video_screen_ptr.is_none() {
                // A later clear won — discard this rebuild.
                order_out_all(built);
                return;
            }
            g.windows = built;
        }
    }
}

fn build_cover_windows(
    video_screen: &objc2::rc::Retained<objc2_app_kit::NSScreen>,
) -> Vec<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSBackingStoreType, NSColor, NSMainMenuWindowLevel, NSScreen, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::NSRect;

    let Some(mtm) = MainThreadMarker::new() else {
        return Vec::new();
    };
    let level = NSMainMenuWindowLevel + 1;
    let video_ptr = objc2::rc::Retained::as_ptr(video_screen);
    let mut slots = Vec::new();
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
        black.orderFrontRegardless();
        slots.push(black);
    }
    slots
}

fn playback_session_active(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_visible: bool,
) -> bool {
    if recent_visible {
        return false;
    }
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    let has_path = b.mpv.get_property::<String>("path").ok().is_some_and(|s| {
        let t = s.trim();
        !t.is_empty() && t != "null" && t != "undefined"
    });
    if !has_path {
        return false;
    }
    let paused = b.mpv.get_property::<bool>("pause").unwrap_or(true);
    !paused || tech_hold_active()
}

fn should_apply(
    bo: &ScreenBlackout,
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_visible: bool,
) -> bool {
    bo.enabled
        && win.is_active()
        && multi_screen()
        && playback_session_active(player, recent_visible)
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
        let Some(main_nswin) = crate::macos_window::nswindow_for_widget(win) else {
            queue_clear(&mut bo);
            return;
        };
        let Some(video_screen) = main_nswin.screen() else {
            queue_clear(&mut bo);
            return;
        };
        let screen_count = screen_count_macos();
        let video_ptr = objc2::rc::Retained::as_ptr(&video_screen);
        if bo.video_screen_ptr == Some(video_ptr)
            && bo.last_screen_count == screen_count
            && (!bo.windows.is_empty() || bo.cover_pending)
        {
            return;
        }
        let old = std::mem::take(&mut bo.windows);
        bo.video_screen_ptr = Some(video_ptr);
        bo.last_screen_count = screen_count;
        bo.cover_pending = true;
        AppkitOp::Rebuild {
            old,
            video: video_screen,
            dest: Rc::clone(bo_rc),
        }
    };
    hop_appkit(op);
}
