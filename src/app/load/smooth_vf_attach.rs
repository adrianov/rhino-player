/// Full Smooth 60 / VapourSynth rebuild whenever mpv reports new media (`FileLoaded`, `path`).
/// Always runs [video_pref::apply_mpv_video] (unlike [smooth_vf_attach_if_playing]) so the `.vpy`
/// graph matches the current clip after Open, drag-drop, sibling EOF advance, and **Previous** / **Next**.
fn smooth_60_full_resync_after_media_change(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl: &gtk::GLArea,
    r: &VideoReapply60,
) {
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

/// Attaches Smooth 60 `vf` when enabled, media is open, playback is **not** paused, speed is ~1.0×,
/// and `vapoursynth` is not already present (e.g. after `loadfile` idle or unpause).
///
/// **`trust_not_paused`**: when `true`, treat playback as **not** paused without reading mpv's `pause`
/// property. The transport layer passes `true` on **`Pause(false)`** because **`get_property("pause")`**
/// can lag the observed unpause briefly — Smooth never reattached.
fn smooth_vf_attach_if_playing(
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    r: VideoReapply60,
    trust_not_paused: bool,
) {
    let paused = |mpv: &libmpv2::Mpv| {
        if trust_not_paused {
            false
        } else {
            mpv.get_property::<bool>("pause").unwrap_or(true)
        }
    };
    {
        let g = player.borrow();
        let Some(b) = g.as_ref() else {
            return;
        };
        if r.vp.borrow().smooth_60
            && video_pref::mpv_has_open_media(&b.mpv)
            && !paused(&b.mpv)
            && video_pref::mvtools_vf_eligible(&b.mpv, None)
            && video_pref::vf_chain_has_vapoursynth(&b.mpv)
        {
            return;
        }
    }
    let mut turn_off = false;
    if let Some(b) = player.borrow().as_ref() {
        if !r.vp.borrow().smooth_60 {
            return;
        }
        if !video_pref::mpv_has_open_media(&b.mpv) {
            return;
        }
        if paused(&b.mpv) {
            return;
        }
        let mut vp = r.vp.borrow_mut();
        if vp.smooth_60
            && video_pref::mvtools_vf_eligible(&b.mpv, None)
            && !video_pref::vf_chain_has_vapoursynth(&b.mpv)
        {
            let a = if trust_not_paused {
                video_pref::apply_mpv_video_after_transport_unpause(b, &mut vp, None)
            } else {
                video_pref::apply_mpv_video(b, &mut vp, None)
            };
            let r2 = if trust_not_paused {
                video_pref::reapply_60_after_transport_unpause(b, &mut vp)
            } else {
                video_pref::reapply_60_if_still_missing(b, &mut vp)
            };
            turn_off = a.smooth_auto_off || r2.smooth_auto_off;
        }
    }
    if turn_off {
        sync_smooth_60_to_off(&r.app);
        show_smooth_setup_dialog(&r.app);
    }
    gl.queue_render();
}
