/// Full Smooth 60 / VapourSynth rebuild whenever mpv reports new media (`FileLoaded`, `path`,
/// debounced unpause / seek tail from transport `schedule_smooth_60_resync_idle`).
/// Runs [video_pref::apply_mpv_video] so the `.vpy` graph matches the current clip after Open,
/// drag-drop, sibling EOF advance, **Previous** / **Next**, and coalesced post-seek / unpause.
fn smooth_60_full_resync_after_media_change(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl: &gtk::GLArea,
    r: &VideoReapply60,
) {
    // [sync_media_decode_row_for_me_budget] also runs when the debounce is *scheduled*;
    // right before apply, mpv often reports stable `video-params` while SQLite still had no
    // **decode_w/h** (or global ME was resolved). [effective_smooth_me_budget_px] reads the DB only,
    // so without this resync the first `vf add` can note one px² and a second resync notes another →
    // redundant `vf clr`/`vf add` + duplicate `.vpy` preset lines.
    sync_media_decode_row_for_me_budget(player);
    let mut turn_off = false;
    if let Some(b) = player.borrow().as_ref() {
        let mut vp = r.vp.borrow_mut();
        let a = video_pref::apply_mpv_video(b, &mut vp, None);
        let r2 = video_pref::reapply_60_if_still_missing(b, &mut vp);
        turn_off = a.smooth_auto_off || r2.smooth_auto_off;
    }
    if turn_off {
        sync_smooth_60_to_off(&r.app);
        show_smooth_setup_dialog(&r.app);
    }
    gl.queue_render();
}
