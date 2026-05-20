#[cfg(target_os = "macos")]
thread_local! {
    static MACOS_DEFER_UNFULLSCREEN: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}

/// Present fullscreen-exit on the libdispatch main queue instead of nested GLib sources.
/// Chained zero-duration GLib timeouts still ran inside `g_main_context_dispatch` and reproduced
/// ~74k-frame AppKit recursion (`_syncToolbarPosition` ↔ `_updateTitlebarContainerViewFrameIfNecessary`)
/// on macOS 26.x.
///
/// **`dispatch_get_main_queue` / `dispatch_async_f`** are resolved with **`dlsym(RTLD_DEFAULT, …)`**
/// so the binary does not link **`libdispatch`** explicitly — Xcode 26 SDK **`ld`** often fails to
/// satisfy **`_dispatch_get_main_queue`** from **`-ldispatch`** under **`-nodefaultlibs`**.
#[cfg(target_os = "macos")]
type DispatchQueueRaw = *mut std::ffi::c_void;

#[cfg(target_os = "macos")]
struct LibDispatchDyn {
    get_main_queue: unsafe extern "C" fn() -> DispatchQueueRaw,
    async_f: unsafe extern "C" fn(
        DispatchQueueRaw,
        *mut std::ffi::c_void,
        unsafe extern "C" fn(*mut std::ffi::c_void),
    ),
}

#[cfg(target_os = "macos")]
fn macos_libdispatch_dyn() -> Option<&'static LibDispatchDyn> {
    use std::sync::OnceLock;
    static D: OnceLock<Option<LibDispatchDyn>> = OnceLock::new();
    D.get_or_init(|| unsafe {
        let g = libc::dlsym(libc::RTLD_DEFAULT, c"dispatch_get_main_queue".as_ptr());
        let a = libc::dlsym(libc::RTLD_DEFAULT, c"dispatch_async_f".as_ptr());
        if g.is_null() || a.is_null() {
            return None;
        }
        Some(LibDispatchDyn {
            get_main_queue: std::mem::transmute::<
                *mut libc::c_void,
                unsafe extern "C" fn() -> DispatchQueueRaw,
            >(g),
            async_f: std::mem::transmute::<
                *mut libc::c_void,
                unsafe extern "C" fn(
                    DispatchQueueRaw,
                    *mut std::ffi::c_void,
                    unsafe extern "C" fn(*mut std::ffi::c_void),
                ),
            >(a),
        })
    })
    .as_ref()
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_unfullscreen_dispatch_final(ctx: *mut std::ffi::c_void) {
    if ctx.is_null() {
        return;
    }
    let widget = glib::translate::from_glib_borrow::<*mut gtk::ffi::GtkWidget, gtk::Widget>(
        ctx.cast(),
    );
    if let Some(win) = widget.downcast_ref::<adw::ApplicationWindow>() {
        if win.is_fullscreen() {
            win.unfullscreen();
        }
    }
    glib::gobject_ffi::g_object_unref(ctx.cast());
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_unfullscreen_dispatch_hop3(ctx: *mut std::ffi::c_void) {
    let Some(d) = macos_libdispatch_dyn() else {
        macos_unfullscreen_dispatch_final(ctx);
        return;
    };
    let q = unsafe { (d.get_main_queue)() };
    unsafe { (d.async_f)(q, ctx, macos_unfullscreen_dispatch_final) };
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_unfullscreen_dispatch_hop2(ctx: *mut std::ffi::c_void) {
    let Some(d) = macos_libdispatch_dyn() else {
        macos_unfullscreen_dispatch_final(ctx);
        return;
    };
    let q = unsafe { (d.get_main_queue)() };
    unsafe { (d.async_f)(q, ctx, macos_unfullscreen_dispatch_hop3) };
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_unfullscreen_dispatch_hop1(ctx: *mut std::ffi::c_void) {
    let Some(d) = macos_libdispatch_dyn() else {
        macos_unfullscreen_dispatch_final(ctx);
        return;
    };
    let q = unsafe { (d.get_main_queue)() };
    unsafe { (d.async_f)(q, ctx, macos_unfullscreen_dispatch_hop2) };
}

#[cfg(target_os = "macos")]
fn macos_unfullscreen_timer_fallback(ctx: *mut std::ffi::c_void) {
    let _ = glib::timeout_add_local_once(Duration::ZERO, move || {
        let _ = glib::timeout_add_local_once(Duration::ZERO, move || {
            let _ = glib::timeout_add_local_once(Duration::ZERO, move || {
                unsafe { macos_unfullscreen_dispatch_final(ctx) };
            });
        });
    });
}

#[cfg(target_os = "macos")]
fn macos_dispatch_chain_then_unfullscreen(win: &adw::ApplicationWindow) {
    let wptr = win.upcast_ref::<gtk::Widget>().as_ptr();
    unsafe {
        glib::gobject_ffi::g_object_ref(wptr.cast());
    }
    let Some(d) = macos_libdispatch_dyn() else {
        macos_unfullscreen_timer_fallback(wptr.cast());
        return;
    };
    let q = unsafe { (d.get_main_queue)() };
    unsafe {
        (d.async_f)(q, wptr.cast(), macos_unfullscreen_dispatch_hop1);
    }
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
                macos_dispatch_chain_then_unfullscreen(&win);
            },
        );
        *slot.borrow_mut() = Some(id);
    });
}
