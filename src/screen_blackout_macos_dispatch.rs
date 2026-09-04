// AppKit-only dispatch plumbing: ops hop to the main queue; never touch GTK/GObject here
// (corrupts GLib idle sources). Include!'d from `screen_blackout_macos.rs`.

/// AppKit work executed on the main dispatch queue.
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
        // Ordering out the covers does not make AppKit re-evaluate the
        // cursor until the pointer moves again; covers own a blank cursor
        // rect, so without this nudge the pointer stays invisible on the
        // just-uncovered display (user pause leaves the mouse parked).
        AppkitOp::Clear(windows) => {
            order_out_all(windows);
            crate::macos_window::show_system_cursor();
        }
        AppkitOp::Rebuild { old, video, dest } => {
            order_out_all(old);
            order_out_all(std::mem::take(&mut dest.borrow_mut().windows));
            let built = build_cover_windows(&video);
            let mut g = dest.borrow_mut();
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
