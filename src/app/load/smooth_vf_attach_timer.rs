/// Schedules Smooth 60 `vf` attach [video_pref::SMOOTH_VF_ATTACH_DELAY_MS] after load or unpause when
/// still playing. Arms [MpvBundle::smooth_vf_not_before] so [video_pref::resync_smooth_if_speed_mismatch]
/// does not attach earlier on the 320 ms file-loaded tick.
fn schedule_smooth_vf_attach_after_delay(
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    r: VideoReapply60,
) {
    // Warm path: already playing with the Smooth `vf` (e.g. resumed after continue list) — skip timer.
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
    let Some(path_when_scheduled) = (|| {
        let g = player.borrow();
        let b = g.as_ref()?;
        if !r.vp.borrow().smooth_60 {
            return None;
        }
        if !video_pref::mpv_has_open_media(&b.mpv) {
            return None;
        }
        let path = b.mpv.get_property::<String>("path").ok()?;
        if path.trim().is_empty() {
            return None;
        }
        if b.mpv.get_property::<bool>("pause").unwrap_or(true) {
            return None;
        }
        let until = std::time::Instant::now()
            + std::time::Duration::from_millis(video_pref::SMOOTH_VF_ATTACH_DELAY_MS);
        b.smooth_vf_not_before.set(Some(until));
        Some(path)
    })() else {
        return;
    };
    let p = Rc::clone(&player);
    let glc = gl.clone();
    let rc = r.clone();
    let _ = glib::timeout_add_local(
        std::time::Duration::from_millis(video_pref::SMOOTH_VF_ATTACH_DELAY_MS),
        move || {
            let mut turn_off = false;
            if let Some(b) = p.borrow().as_ref() {
                b.smooth_vf_not_before.set(None);
                if b.mpv.get_property::<bool>("pause").unwrap_or(true) {
                    glc.queue_render();
                    return glib::ControlFlow::Break;
                }
                let cur = b.mpv.get_property::<String>("path").ok().unwrap_or_default();
                if cur != path_when_scheduled {
                    glc.queue_render();
                    return glib::ControlFlow::Break;
                }
                let mut vp = rc.vp.borrow_mut();
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
                sync_smooth_60_to_off(&rc.app);
                show_smooth_setup_dialog(&rc.app);
            }
            glc.queue_render();
            glib::ControlFlow::Break
        },
    );
}

/// Schedules one idle after `loadfile`: [video_pref::apply_mpv_fast_start_after_load], then a timer
/// that attaches the VapourSynth `vf` if still playing after [video_pref::SMOOTH_VF_ATTACH_DELAY_MS].
fn schedule_reapply_60(player: &Rc<RefCell<Option<MpvBundle>>>, gl: &gtk::GLArea, o: &LoadOpts) {
    let Some(r) = o.reapply_60.as_ref() else { return };
    let p = Rc::clone(player);
    let r0 = r.clone();
    let gl0 = gl.clone();
    let _ = glib::idle_add_local_once(move || {
        if let Some(b) = p.borrow().as_ref() {
            let a = video_pref::apply_mpv_fast_start_after_load(b, &mut r0.vp.borrow_mut());
            if a.smooth_auto_off {
                sync_smooth_60_to_off(&r0.app);
                show_smooth_setup_dialog(&r0.app);
            }
        }
        gl0.queue_render();
        schedule_smooth_vf_attach_after_delay(Rc::clone(&p), gl0.clone(), r0.clone());
    });
}
