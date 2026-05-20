#[cfg(target_os = "macos")]
thread_local! {
    static MACOS_DEFER_UNFULLSCREEN: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}

/// Present fullscreen-exit on the libdispatch main queue instead of nested GLib sources.
/// Zero-duration GLib timeout chains still run inside `g_main_context_dispatch` and reproduced
/// ~74k-frame AppKit recursion (`_syncToolbarPosition` ↔ `_updateTitlebarContainerViewFrameIfNecessary`)
/// on macOS 26.x when paired with GTK `unfullscreen`.
#[cfg(target_os = "macos")]
type DispatchQueueRaw = *mut std::ffi::c_void;

#[cfg(target_os = "macos")]
type DispatchTime = u64;

#[cfg(target_os = "macos")]
const DISPATCH_TIME_NOW: DispatchTime = 0;

/// Delay before re-checking AppKit vs GTK fullscreen state after native exit starts.
#[cfg(target_os = "macos")]
const MACOS_FS_SYNC_DELAY_NS: i64 = 350_000_000;

#[cfg(target_os = "macos")]
struct LibDispatch {
    get_main_queue: unsafe extern "C" fn() -> DispatchQueueRaw,
    async_f: unsafe extern "C" fn(
        DispatchQueueRaw,
        *mut std::ffi::c_void,
        unsafe extern "C" fn(*mut std::ffi::c_void),
    ),
    time: unsafe extern "C" fn(DispatchTime, i64) -> DispatchTime,
    after_f: unsafe extern "C" fn(
        DispatchTime,
        DispatchQueueRaw,
        *mut std::ffi::c_void,
        unsafe extern "C" fn(*mut std::ffi::c_void),
    ),
}

#[cfg(target_os = "macos")]
fn macos_libdispatch() -> Option<&'static LibDispatch> {
    use std::sync::OnceLock;
    static D: OnceLock<Option<LibDispatch>> = OnceLock::new();
    D.get_or_init(macos_load_libdispatch).as_ref()
}

#[cfg(target_os = "macos")]
fn macos_load_libdispatch() -> Option<LibDispatch> {
    unsafe {
        let mut handle = libc::RTLD_DEFAULT;
        let path = c"/usr/lib/system/libdispatch.dylib";
        let lib = libc::dlopen(path.as_ptr(), libc::RTLD_LAZY);
        if !lib.is_null() {
            handle = lib;
        }
        let g = libc::dlsym(handle, c"dispatch_get_main_queue".as_ptr());
        let a = libc::dlsym(handle, c"dispatch_async_f".as_ptr());
        let t = libc::dlsym(handle, c"dispatch_time".as_ptr());
        let f = libc::dlsym(handle, c"dispatch_after_f".as_ptr());
        if g.is_null() || a.is_null() || t.is_null() || f.is_null() {
            return None;
        }
        Some(LibDispatch {
            get_main_queue: std::mem::transmute(g),
            async_f: std::mem::transmute(a),
            time: std::mem::transmute(t),
            after_f: std::mem::transmute(f),
        })
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_unfullscreen_sync_fallback(ctx: *mut std::ffi::c_void) {
    if ctx.is_null() {
        return;
    }
    let widget = glib::translate::from_glib_borrow::<*mut gtk::ffi::GtkWidget, gtk::Widget>(
        ctx.cast(),
    );
    if let Some(win) = widget.downcast_ref::<adw::ApplicationWindow>() {
        crate::macos_window::sync_gtk_fullscreen_from_native(win);
    }
    glib::gobject_ffi::g_object_unref(ctx.cast());
}

#[cfg(target_os = "macos")]
unsafe fn macos_schedule_unfullscreen_sync(ctx: *mut std::ffi::c_void, d: &LibDispatch) {
    let q = (d.get_main_queue)();
    let when = (d.time)(DISPATCH_TIME_NOW, MACOS_FS_SYNC_DELAY_NS);
    (d.after_f)(when, q, ctx, macos_unfullscreen_sync_fallback);
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_unfullscreen_dispatch_final(ctx: *mut std::ffi::c_void) {
    if ctx.is_null() {
        return;
    }
    let widget = glib::translate::from_glib_borrow::<*mut gtk::ffi::GtkWidget, gtk::Widget>(
        ctx.cast(),
    );
    let Some(win) = widget.downcast_ref::<adw::ApplicationWindow>() else {
        glib::gobject_ffi::g_object_unref(ctx.cast());
        return;
    };
    if !win.is_fullscreen() {
        glib::gobject_ffi::g_object_unref(ctx.cast());
        return;
    }
    if let Some(nswin) =
        crate::macos_window::nswindow_for_widget(win.upcast_ref::<gtk::Widget>())
    {
        if !crate::macos_window::ns_window_is_native_fullscreen(&nswin) {
            win.set_fullscreened(false);
            glib::gobject_ffi::g_object_unref(ctx.cast());
            return;
        }
        let _ = crate::macos_window::native_leave_fullscreen(&nswin);
        let _ = crate::macos_window::post_fullscreen_toggle_shortcut(&nswin);
    } else {
        win.set_fullscreened(false);
        glib::gobject_ffi::g_object_unref(ctx.cast());
        return;
    }
    let Some(d) = macos_libdispatch() else {
        let w = win.clone();
        let _ = glib::timeout_add_local_once(
            Duration::from_millis(MACOS_FS_SYNC_DELAY_NS as u64 / 1_000_000),
            move || crate::macos_window::sync_gtk_fullscreen_from_native(&w),
        );
        glib::gobject_ffi::g_object_unref(ctx.cast());
        return;
    };
    macos_schedule_unfullscreen_sync(ctx, d);
}

#[cfg(target_os = "macos")]
fn macos_dispatch_async_main(
    ctx: *mut std::ffi::c_void,
    work: unsafe extern "C" fn(*mut std::ffi::c_void),
) {
    let Some(d) = macos_libdispatch() else {
        let _ = glib::timeout_add_local_once(Duration::from_millis(1), move || {
            unsafe { work(ctx) };
        });
        return;
    };
    unsafe {
        (d.async_f)((d.get_main_queue)(), ctx, work);
    }
}

#[cfg(target_os = "macos")]
fn macos_dispatch_then_unfullscreen(win: &adw::ApplicationWindow) {
    let wptr = win.upcast_ref::<gtk::Widget>().as_ptr();
    unsafe {
        glib::gobject_ffi::g_object_ref(wptr.cast());
    }
    macos_dispatch_async_main(wptr.cast(), macos_unfullscreen_dispatch_final);
}

#[cfg(target_os = "macos")]
fn macos_schedule_unfullscreen(win: adw::ApplicationWindow) {
    MACOS_DEFER_UNFULLSCREEN.with(|slot| {
        drop_glib_source(slot);
        let id = glib::timeout_add_local_once(
            crate::fullscreen_timing::TRANSITION_SETTLE,
            move || {
                MACOS_DEFER_UNFULLSCREEN.with(|s| {
                    *s.borrow_mut() = None;
                });
                macos_dispatch_then_unfullscreen(&win);
            },
        );
        *slot.borrow_mut() = Some(id);
    });
}
