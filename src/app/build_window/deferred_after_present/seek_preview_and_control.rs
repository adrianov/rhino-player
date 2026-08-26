/// Seek-bar hover preview when enabled at startup; inert sinks for the seek wiring otherwise.
fn connect_seek_preview_after_present(
    args: &WindowAfterPresentArgs,
) -> (
    Rc<Cell<f64>>,
    Rc<RefCell<Option<crate::mpv_embed::MpvPreviewGl>>>,
) {
    if !args.seek_bar_on.get() {
        return (Rc::new(Cell::new(0.0)), Rc::new(RefCell::new(None)));
    }
    let seek_preview = seek_bar_preview::connect(
        &args.w.seek,
        &args.w.seek_adj,
        seek_bar_preview::SeekPreviewCells {
            player: Rc::clone(&args.player),
            last_path: Rc::clone(&args.last_path),
            enabled: Rc::clone(&args.seek_bar_on),
            chapters: Rc::clone(&args.seek_chapters),
            dvd_bar: Rc::clone(&args.dvd_bar),
        },
        wap_seek_preview_ctx(args),
    );
    args.w.outer_ovl.add_overlay(&seek_preview.container);
    (
        Rc::clone(&seek_preview.hover_t),
        Rc::clone(&seek_preview.preview),
    )
}

/// Overlay placement context for the seek preview.
fn wap_seek_preview_ctx(args: &WindowAfterPresentArgs) -> seek_bar_preview::SeekPreviewCtx {
    seek_bar_preview::SeekPreviewCtx {
        ovl: args.w.outer_ovl.clone(),
        // Lift above the chrome that is actually in the toolbar (shell on macOS).
        #[cfg(target_os = "macos")]
        bottom: args.w.bottom_shell.clone(),
        #[cfg(not(target_os = "macos"))]
        bottom: args.w.bottom.clone(),
    }
}

/// Seek bar drag / release / keyboard seeking on the main player.
fn wire_seek_control_step(
    args: &WindowAfterPresentArgs,
    preview_hover_t: Rc<Cell<f64>>,
    preview_player: Rc<RefCell<Option<crate::mpv_embed::MpvPreviewGl>>>,
) {
    wire_seek_control(
        &args.w.seek,
        SeekControlDeps {
            player: args.player.clone(),
            preview_player,
            gl: args.w.gl_area.clone(),
            seek_sync: args.seek_sync.clone(),
            seek_grabbed: args.seek_grabbed.clone(),
            seek_preview_on: Rc::clone(&args.seek_bar_on),
            time_left: args.w.time_left.clone(),
            preview_hover_t,
            smooth_seek_debounce: args.smooth_seek_debounce.clone(),
            resume_after_seek_idle: args.resume_after_seek_idle.clone(),
            play_toggle: args.play_ctx.clone(),
            dvd_bar: Rc::clone(&args.dvd_bar),
        },
    );
}
