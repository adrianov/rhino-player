// macOS libdispatch hop for the programmatic fullscreen exit: ref-count the window,
// schedule toggleFullScreen: on the main queue, and re-validate arming guards when it runs.
//
// Included from `chrome_macos_unfullscreen_defer.rs`; shares its module scope.

#[cfg(target_os = "macos")]
use core::ffi::c_void;

#[cfg(target_os = "macos")]
type DispatchQueue = *const c_void;

#[cfg(target_os = "macos")]
extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(
        queue: DispatchQueue,
        context: *mut c_void,
        work: unsafe extern "C" fn(*mut c_void),
    );
}

#[cfg(target_os = "macos")]
struct ExitToggleCtx {
    widget: *mut gtk::ffi::GtkWidget,
    gen: u64,
    retry: u8,
}

#[cfg(target_os = "macos")]
fn macos_dispatch_toggle_exit(win: &adw::ApplicationWindow, gen: u64, retry: u8) -> bool {
    if !crate::macos_window::ns_fullscreen_for_win(win) {
        return false;
    }
    let widget = win.upcast_ref::<gtk::Widget>().as_ptr();
    unsafe {
        glib::gobject_ffi::g_object_ref(widget.cast());
    }
    let ctx = Box::into_raw(Box::new(ExitToggleCtx { widget, gen, retry }));
    crate::macos_fs_debug::log("dispatch_async toggleFullScreen exit");
    unsafe {
        dispatch_async_f(
            &_dispatch_main_q as *const c_void,
            ctx.cast(),
            macos_exit_toggle_dispatch,
        );
    }
    true
}

/// Re-check arming guards on the dispatch thread; true → stop without toggling.
#[cfg(target_os = "macos")]
fn exit_dispatch_blocked(win: &adw::ApplicationWindow, gen: u64) -> bool {
    if !crate::macos_fs_exit::unfs_gen_is_current(gen) || !crate::macos_fs_exit::exit_armed() {
        return true;
    }
    if crate::macos_window::clear_stale_gtk_fullscreen(win) {
        return true;
    }
    if !crate::macos_window::window_still_fullscreen(win) {
        crate::macos_fs_exit::clear_exit();
        return true;
    }
    false
}

/// Transition started between schedule and run — reuse the capped GLib poll path.
#[cfg(target_os = "macos")]
fn rearm_step_after_race(win: &adw::ApplicationWindow, gen: u64, retry: u8) {
    let win2 = win.clone();
    let _ = glib::timeout_add_local_once(MACOS_FS_TRANSITION_POLL, move || {
        macos_unfullscreen_step(win2, gen, retry.saturating_add(1));
    });
}

#[cfg(target_os = "macos")]
fn toggle_nswindow_fullscreen(gtk: &gtk::Widget, gen: u64) {
    let Some(nswin) = crate::macos_window::nswindow_for_widget(gtk) else {
        eprintln!("[rhino] macos-fs: dispatch exit: no NSWindow gen={gen}");
        crate::macos_fs_exit::clear_exit();
        return;
    };
    crate::macos_window::prep_native_fullscreen_exit(&nswin);
    crate::macos_fs_debug::log("toggleFullScreen (dispatch)");
    unsafe {
        let _: () = objc2::msg_send![&*nswin, toggleFullScreen: &*nswin];
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_exit_toggle_dispatch(raw: *mut c_void) {
    let ctx = unsafe { Box::from_raw(raw.cast::<ExitToggleCtx>()) };
    let widget = unsafe {
        glib::translate::from_glib_full::<*mut gtk::ffi::GtkWidget, gtk::Widget>(ctx.widget)
    };
    let Some(win) = widget.downcast_ref::<adw::ApplicationWindow>() else {
        eprintln!("[rhino] macos-fs: dispatch exit: not ApplicationWindow");
        return;
    };
    if exit_dispatch_blocked(win, ctx.gen) {
        return;
    }
    let gtk = win.upcast_ref::<gtk::Widget>();
    if crate::macos_window::gdk_macos_in_fullscreen_transition(gtk) {
        rearm_step_after_race(win, ctx.gen, ctx.retry);
        return;
    }
    toggle_nswindow_fullscreen(gtk, ctx.gen);
}
