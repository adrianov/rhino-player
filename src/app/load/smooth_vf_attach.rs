/// Attaches Smooth 60 `vf` when enabled, media is open, playback is **not** paused, speed is ~1.0×,
/// and `vapoursynth` is not already present (e.g. after `loadfile` idle or unpause).
fn smooth_vf_attach_if_playing(player: Rc<RefCell<Option<MpvBundle>>>, gl: gtk::GLArea, r: VideoReapply60) {
    {
        let g = player.borrow();
        let Some(b) = g.as_ref() else {
            return;
        };
        if r.vp.borrow().smooth_60
            && video_pref::mpv_has_open_media(&b.mpv)
            && !b.mpv.get_property::<bool>("pause").unwrap_or(true)
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
        if b.mpv.get_property::<bool>("pause").unwrap_or(true) {
            return;
        }
        let mut vp = r.vp.borrow_mut();
        if vp.smooth_60
            && video_pref::mvtools_vf_eligible(&b.mpv, None)
            && !video_pref::vf_chain_has_vapoursynth(&b.mpv)
        {
            let a = video_pref::apply_mpv_video(b, &mut vp, None);
            let r2 = video_pref::reapply_60_if_still_missing(b, &mut vp);
            turn_off = a.smooth_auto_off || r2.smooth_auto_off;
        }
    }
    if turn_off {
        sync_smooth_60_to_off(&r.app);
        show_smooth_setup_dialog(&r.app);
    }
    gl.queue_render();
}

/// One GLib idle after `loadfile`: full Smooth apply via [video_pref::apply_mpv_video] when appropriate.
fn schedule_reapply_60(player: &Rc<RefCell<Option<MpvBundle>>>, gl: &gtk::GLArea, o: &LoadOpts) {
    let Some(r) = o.reapply_60.as_ref() else { return };
    let p = Rc::clone(player);
    let r0 = r.clone();
    let gl0 = gl.clone();
    let _ = glib::idle_add_local_once(move || {
        let mut turn_off = false;
        if let Some(b) = p.borrow().as_ref() {
            let mut vp = r0.vp.borrow_mut();
            let a = video_pref::apply_mpv_video(b, &mut vp, None);
            let r2 = video_pref::reapply_60_if_still_missing(b, &mut vp);
            turn_off = a.smooth_auto_off || r2.smooth_auto_off;
        }
        if turn_off {
            sync_smooth_60_to_off(&r0.app);
            show_smooth_setup_dialog(&r0.app);
        }
        gl0.queue_render();
    });
}
