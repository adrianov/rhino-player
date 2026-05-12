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
        turn_off = a.smooth_auto_off;
    }
    if turn_off {
        sync_smooth_60_to_off(&r.app);
        show_smooth_setup_dialog(&r.app);
        gl.queue_render();
        return;
    }
    // [reapply_60_if_still_missing] reads `vf` after a successful `vf add`. libmpv can accept the
    // command in the same main-loop slice before `get_property("vf")` reflects the new chain —
    // running it synchronously here caused a second [apply_mpv_video] + duplicate VapourSynth init on seek.
    // `GLArea::queue_render` runs once in that idle (not here too): the next slice sees settled `vf`
    // and any rare reattach from [reapply_60_if_still_missing], without double-invalidating for one resync.
    let player2 = Rc::clone(player);
    let r2 = r.clone();
    let gl2 = gl.clone();
    let _ = glib::idle_add_local_once(move || {
        let mut t = false;
        if let Some(b) = player2.borrow().as_ref() {
            let mut vp = r2.vp.borrow_mut();
            let rx = video_pref::reapply_60_if_still_missing(b, &mut vp);
            t = rx.smooth_auto_off;
        }
        if t {
            sync_smooth_60_to_off(&r2.app);
            show_smooth_setup_dialog(&r2.app);
        }
        gl2.queue_render();
    });
}
