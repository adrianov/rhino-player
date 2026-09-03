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
        seek_preview_cells(args),
        wap_seek_preview_ctx(args),
    );
    #[cfg(not(target_os = "macos"))]
    args.w.outer_ovl.add_overlay(&seek_preview.container);
    (
        Rc::clone(&seek_preview.hover_t),
        Rc::clone(&seek_preview.preview),
    )
}

fn seek_preview_cells(args: &WindowAfterPresentArgs) -> seek_bar_preview::SeekPreviewCells {
    seek_bar_preview::SeekPreviewCells {
        player: Rc::clone(&args.player),
        last_path: Rc::clone(&args.last_path),
        enabled: Rc::clone(&args.seek_bar_on),
        recent_visible: Rc::clone(&args.recent_visible),
        chapters: Rc::clone(&args.seek_chapters),
        dvd_bar: Rc::clone(&args.dvd_bar),
    }
}

/// Overlay placement context for the seek preview.
fn wap_seek_preview_ctx(_args: &WindowAfterPresentArgs) -> seek_bar_preview::SeekPreviewCtx {
    seek_bar_preview::SeekPreviewCtx {
        #[cfg(not(target_os = "macos"))]
        ovl: _args.w.outer_ovl.clone(),
        #[cfg(not(target_os = "macos"))]
        bottom: _args.w.bottom.clone(),
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
