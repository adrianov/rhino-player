// macOS programmatic fullscreen exit: arm guard, settle, hop to libdispatch, then toggleFullScreen:.
// Do not reveal ToolbarView bars while the native fullscreen mask is set (titlebar layout recursion).
// Not set_fullscreened(false). Coalesced via macos_fs_exit; clear_exit when windowed (leave restore).
//
// toggleFullScreen: must not run inside a GLib idle/timeout trampoline — that nests GDK layout into
// _NSExitFullScreenTransitionController and can stack-overflow on macOS 26.x.

#[cfg(target_os = "macos")]
use glib::prelude::ObjectType;
#[cfg(target_os = "macos")]
use std::ffi::c_void;

#[cfg(target_os = "macos")]
const MACOS_FS_TRANSITION_POLL: std::time::Duration = std::time::Duration::from_millis(80);

#[cfg(target_os = "macos")]
const MACOS_FS_TRANSITION_POLL_MAX: u8 = 12;

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
fn macos_unfullscreen_step(win: adw::ApplicationWindow, gen: u64, retry: u8) {
    // Stale gen: a newer arm owns exit_armed — do not clear it.
    if !crate::macos_fs_exit::unfs_gen_is_current(gen) {
        return;
    }
    if crate::macos_window::clear_stale_gtk_fullscreen(&win) {
        return;
    }
    if !crate::macos_window::window_still_fullscreen(&win) {
        crate::macos_fs_exit::clear_exit();
        return;
    }
    let gtk = win.upcast_ref::<gtk::Widget>();
    if crate::macos_window::gdk_macos_in_fullscreen_transition(gtk) {
        if retry < MACOS_FS_TRANSITION_POLL_MAX {
            let win2 = win.clone();
            let _ = glib::timeout_add_local_once(MACOS_FS_TRANSITION_POLL, move || {
                macos_unfullscreen_step(win2, gen, retry.saturating_add(1));
            });
            return;
        }
        eprintln!(
            "[rhino] macos-fs: exit gave up — still inFullscreenTransition gen={gen} retry={retry}"
        );
        crate::macos_fs_exit::clear_exit();
        return;
    }
    if !macos_dispatch_toggle_exit(&win, gen, retry) {
        eprintln!(
            "[rhino] macos-fs: dispatch exit schedule failed gen={gen} retry={retry} gtk={} ns={}",
            win.is_fullscreen(),
            crate::macos_window::ns_fullscreen_for_win(&win),
        );
        let _ = crate::macos_window::clear_stale_gtk_fullscreen(&win);
        crate::macos_fs_exit::clear_exit();
    }
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
    let ctx = Box::into_raw(Box::new(ExitToggleCtx {
        widget,
        gen,
        retry,
    }));
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
    if !crate::macos_fs_exit::unfs_gen_is_current(ctx.gen) || !crate::macos_fs_exit::exit_armed()
    {
        return;
    }
    if crate::macos_window::clear_stale_gtk_fullscreen(win) {
        return;
    }
    if !crate::macos_window::window_still_fullscreen(win) {
        crate::macos_fs_exit::clear_exit();
        return;
    }
    let gtk = win.upcast_ref::<gtk::Widget>();
    if crate::macos_window::gdk_macos_in_fullscreen_transition(gtk) {
        // Race: transition started after schedule — reuse the capped GLib poll path.
        let win2 = win.clone();
        let gen = ctx.gen;
        let retry = ctx.retry;
        let _ = glib::timeout_add_local_once(MACOS_FS_TRANSITION_POLL, move || {
            macos_unfullscreen_step(win2, gen, retry.saturating_add(1));
        });
        return;
    }
    let Some(nswin) = crate::macos_window::nswindow_for_widget(gtk) else {
        eprintln!("[rhino] macos-fs: dispatch exit: no NSWindow gen={}", ctx.gen);
        crate::macos_fs_exit::clear_exit();
        return;
    };
    crate::macos_fs_debug::log("toggleFullScreen (dispatch)");
    unsafe {
        let _: () = objc2::msg_send![&*nswin, toggleFullScreen: &*nswin];
    }
}

#[cfg(target_os = "macos")]
pub(super) fn macos_schedule_unfullscreen(win: adw::ApplicationWindow) {
    if crate::macos_window::clear_stale_gtk_fullscreen(&win) {
        return;
    }
    if !crate::macos_window::window_still_fullscreen(&win) {
        return;
    }
    let Some(gen) = crate::macos_fs_exit::try_arm_exit() else {
        return;
    };
    macos_traffic_cancel_poll();
    crate::macos_fs_debug::log_win_state("schedule_unfullscreen", &win);
    let win2 = win.clone();
    // Settle past the click/gesture layout burst before hopping to libdispatch.
    let _ = glib::timeout_add_local_once(crate::fullscreen_timing::TRANSITION_SETTLE, move || {
        macos_unfullscreen_step(win2, gen, 0);
    });
}
