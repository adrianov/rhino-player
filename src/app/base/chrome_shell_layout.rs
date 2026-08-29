// After programmatic window resize (VOB / DVD fit-on-open), gdk-macos needs the same
// relayout + layer invalidation as a manual resize or fullscreen focus return.
// One immediate sync + one delayed settle — stacking more passes flashes chrome.

thread_local! {
    static SHELL_LAYOUT: RefCell<Option<Rc<ShellLayoutCtx>>> = const { RefCell::new(None) };
    /// Pending settle for Linux [`schedule_shell_layout_sync`]; replaced on re-entry.
    #[cfg(not(target_os = "macos"))]
    static SHELL_SYNC_SETTLE: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}

/// Widget refs for shell relayout after geometry changes (registered once when attached).
pub(crate) struct ShellLayoutCtx {
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    video_handle: gtk::WindowHandle,
    gl: gtk::GLArea,
    bottom: gtk::Box,
    #[cfg(target_os = "macos")]
    bottom_shell: gtk::Box,
    recent: gtk::Box,
    bar_show: Rc<Cell<bool>>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    touch_chrome: RefCell<Option<Rc<dyn Fn()>>>,
}

pub(crate) fn register_shell_layout(ctx: Rc<ShellLayoutCtx>) {
    SHELL_LAYOUT.with(|s| *s.borrow_mut() = Some(ctx));
}

pub(crate) fn wire_shell_layout_chrome(touch: Rc<dyn Fn()>) {
    SHELL_LAYOUT.with(|s| {
        if let Some(ctx) = s.borrow().as_ref() {
            *ctx.touch_chrome.borrow_mut() = Some(touch);
        }
    });
}

fn toolbar_show(ctx: &ShellLayoutCtx) -> bool {
    ctx.recent.is_visible() || ctx.bar_show.get()
}

// macOS-only resync helpers live in the sibling unit included below.
#[cfg(target_os = "macos")]
include!("chrome_shell_layout_macos_resync.rs");

/// Queue layout (allocate / video render). Platform draws live in [`repaint_shell_layers`].
fn queue_shell_relayout(ctx: &ShellLayoutCtx) {
    ctx.win.queue_resize();
    ctx.root.queue_allocate();
    ctx.video_handle.queue_draw();
    ctx.gl.queue_render();
}

/// Repaint platform chrome layers: opaque bottom bar + gdk compositing on macOS,
/// native surface render elsewhere.
fn repaint_shell_layers(ctx: &ShellLayoutCtx) {
    #[cfg(target_os = "macos")]
    {
        crate::macos_window::refresh_gdk_shell_compositing(
            &ctx.win,
            &ctx.gl,
            &ctx.header,
            &ctx.root,
            &ctx.bottom_shell,
            &ctx.bottom,
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        use gtk::prelude::NativeExt;

        ctx.root.queue_draw();
        ctx.header.queue_draw();
        ctx.bottom.queue_draw();
        ctx.video_handle.queue_draw();
        ctx.win.queue_draw();
        if let Some(surf) = ctx.win.native().and_then(|n| n.surface()) {
            surf.queue_render();
        }
    }
}

/// Nudge mpv's shell layout and refit subtitle position to the new toolbar heights.
fn apply_shell_sub_pos(ctx: &ShellLayoutCtx, show: bool) {
    if let Ok(g) = ctx.player.try_borrow() {
        if let Some(b) = g.as_ref() {
            b.nudge_shell_layout_after_resize(&ctx.gl);
            sub_prefs::apply_sub_pos_for_toolbar(
                &b.mpv,
                show,
                ctx.bottom.height(),
                ctx.gl.height(),
            );
        }
    }
}

fn log_shell_layout(ctx: &ShellLayoutCtx, tag: &str, show: bool) {
    crate::shell_debug_log::log_toolbar_layout(
        tag,
        &crate::shell_debug_log::ToolbarLayoutRefs {
            win: &ctx.win,
            root: &ctx.root,
            header: &ctx.header,
            bottom: &ctx.bottom,
            gl: &ctx.gl,
            #[cfg(target_os = "macos")]
            bottom_shell: &ctx.bottom_shell,
        },
        ctx.recent.is_visible(),
        ctx.bar_show.get(),
        show,
    );
}

fn sync_shell_layout_tag(ctx: &ShellLayoutCtx, tag: &str) {
    let show = toolbar_show(ctx);
    let _ = set_toolbar_reveal(&ctx.root, show);
    queue_shell_relayout(ctx);
    repaint_shell_layers(ctx);
    apply_shell_sub_pos(ctx, show);
    log_shell_layout(ctx, tag, show);
}

/// Arm the single delayed settle; cancels any prior settle first.
#[cfg(not(target_os = "macos"))]
fn arm_shell_sync_settle(ctx: Rc<ShellLayoutCtx>) {
    SHELL_SYNC_SETTLE.with(drop_glib_source);
    let id = glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
        SHELL_SYNC_SETTLE.with(crate::glib_source_drop::finish_glib_source);
        sync_shell_layout_tag(&ctx, "sched-150ms");
        if let Some(touch) = ctx.touch_chrome.borrow().clone() {
            touch();
        }
    });
    SHELL_SYNC_SETTLE.with(|slot| *slot.borrow_mut() = Some(id));
}

/// Immediate sync + one delayed settle. macOS fit/hide paths use sync+nudge instead.
#[cfg(not(target_os = "macos"))]
pub(crate) fn schedule_shell_layout_sync() {
    let Some(ctx) = SHELL_LAYOUT.with(|s| s.borrow().clone()) else {
        return;
    };
    sync_shell_layout_tag(&ctx, "sched-0");
    arm_shell_sync_settle(ctx);
}
