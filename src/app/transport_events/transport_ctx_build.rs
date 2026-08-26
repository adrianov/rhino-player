// Construction of [TransportCtx] (and its EOF sub-context) from [TransportSetup].

fn build_transport_ctx(s: TransportSetup) -> Rc<TransportCtx> {
    let TransportSetup {
        app,
        sub_pref,
        win,
        gl,
        recent,
        last_path,
        sibling_seof,
        exit_after_current,
        win_aspect,
        idle_inhib,
        mpv_teardown_after_draw,
        on_video_chrome,
        on_file_loaded,
        reapply_60,
        hdr_title_mirror,
        playback_focus,
        on_open_fail,
        player,
        video_pref,
        widgets,
        bar_show,
        recent_visible,
        sibling_nav,
        continue_grid_cache,
        seek_chapters,
        dvd_bar,
        blackout,
    } = s;
    let eof = TransportEofCtx {
        app,
        sub_pref,
        win,
        gl,
        recent,
        last_path,
        sibling_seof,
        exit_after_current,
        win_aspect,
        idle_inhib,
        mpv_teardown_after_draw,
        on_video_chrome,
        on_file_loaded,
        reapply_60,
        hdr_title_mirror,
        playback_focus,
        on_open_fail,
    };
    Rc::new(TransportCtx {
        player,
        widgets,
        eof,
        video_pref,
        smooth_budget_decoder: fresh_smooth_budget_decoder(),
        bar_show,
        recent_visible,
        sibling_nav,
        idle_resync_pending: Rc::new(Cell::new(false)),
        smooth_60_resync_debounce: Rc::new(RefCell::new(None)),
        tick: Rc::new(RefCell::new(None)),
        cache: Rc::new(RefCell::new(TransportCache::default())),
        continue_grid_cache,
        seek_chapters,
        dvd_bar,
        blackout,
    })
}

/// Per-open Smooth decoder-budget state; replaced wholesale on every `FileLoaded`.
fn fresh_smooth_budget_decoder() -> Rc<RefCell<crate::video_pref::SmoothBudgetDecoderState>> {
    Rc::new(RefCell::new(
        crate::video_pref::SmoothBudgetDecoderState::default(),
    ))
}
