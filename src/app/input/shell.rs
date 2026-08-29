/// Packs toolbar + video stack into the application window (safe to call once).
fn attach_window_shell(s: &WindowInputShell) {
    s.root.add_top_bar(&s.header);
    s.root.set_content(Some(&s.video_handle));
    #[cfg(target_os = "macos")]
    s.root.add_bottom_bar(&s.bottom_shell);
    #[cfg(not(target_os = "macos"))]
    s.root.add_bottom_bar(&s.bottom);
    s.outer_ovl.set_child(Some(&s.root));
    s.win.set_content(Some(&s.outer_ovl));
}

fn w_in_set_shell(ctx: &WindowInputCtx) {
    if ctx.shell.win.content().is_some() {
        return;
    }
    attach_window_shell(&ctx.shell);
}

include!("shell_fs_clock.rs");
include!("shell_fs_notify.rs");
include!("shell_fullscreen.rs");

include!("shell_fs_restore.rs");

fn schedule_sub_pos(
    gl: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    show: bool,
    bot_h: i32,
) {
    gl.queue_render();
    if let Some(bundle) = player.borrow().as_ref() {
        sub_prefs::apply_sub_pos_for_toolbar(&bundle.mpv, show, bot_h, gl.height());
    }
}
