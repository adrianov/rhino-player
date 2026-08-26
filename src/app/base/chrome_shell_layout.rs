// After programmatic window resize (VOB / DVD fit-on-open), gdk-macos needs the same
// relayout + layer invalidation as a manual resize or fullscreen focus return.

thread_local! {
    static SHELL_LAYOUT: RefCell<Option<Rc<ShellLayoutCtx>>> = const { RefCell::new(None) };
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

/// Queue relayout across every shell widget after a geometry change.
fn queue_shell_relayout(ctx: &ShellLayoutCtx) {
    ctx.win.queue_resize();
    ctx.root.queue_allocate();
    ctx.root.queue_draw();
    ctx.header.queue_draw();
    ctx.bottom.queue_draw();
    #[cfg(target_os = "macos")]
    ctx.bottom_shell.queue_draw();
    ctx.video_handle.queue_draw();
    ctx.gl.queue_render();
}

/// Repaint platform chrome layers: opaque bottom bar + gdk compositing on macOS,
/// native surface render elsewhere.
fn repaint_shell_layers(ctx: &ShellLayoutCtx) {
    #[cfg(target_os = "macos")]
    {
        crate::macos_bottom_bar::repaint_opaque(&ctx.bottom_shell, &ctx.bottom);
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

/// One delayed [`sync_shell_layout_tag`] pass with a log tag.
fn shell_sync_after_delay(ctx: &Rc<ShellLayoutCtx>, delay_ms: u64, tag: &'static str) {
    let c = Rc::clone(ctx);
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(delay_ms), move || {
        sync_shell_layout_tag(&c, tag);
    });
}

/// Idle + short delays so ToolbarView bottom bar lands after NSWindow / revealer layout.
pub(crate) fn schedule_shell_layout_sync() {
    let Some(ctx) = SHELL_LAYOUT.with(|s| s.borrow().clone()) else {
        return;
    };
    sync_shell_layout_tag(&ctx, "sched-0");
    let c1 = Rc::clone(&ctx);
    let _ = glib::idle_add_local_once(move || sync_shell_layout_tag(&c1, "sched-idle"));
    let c3 = Rc::clone(&ctx);
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
        sync_shell_layout_tag(&c3, "sched-150ms");
        if let Some(touch) = c3.touch_chrome.borrow().clone() {
            touch();
        }
    });
    shell_sync_after_delay(&ctx, 300, "sched-300ms");
}
