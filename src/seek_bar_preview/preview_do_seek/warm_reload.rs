/// Clears the framebuffer, prepares the auxiliary player, and replaces its file.
/// Returns false (with a warning) when the loadfile command fails.
fn load_into_preview(st: &SeekPreviewState, pr: &mut MpvPreviewGl, load_s: &str) -> bool {
    pr.clear_framebuffer(&st.gl);
    prepare_preview_player(&pr.mpv, load_s);
    if let Err(e) = pr.mpv.command("loadfile", &[load_s, "replace"]) {
        crate::preview_debug::warn(format!("loadfile failed: {e:?} ({load_s})"));
        return false;
    }
    crate::preview_debug::info(format!(
        "loadfile ok ({})",
        crate::preview_debug::mpv_line(&pr.mpv)
    ));
    true
}

/// Persists what the aux player now has loaded.
fn store_loaded_state(
    st: &SeekPreviewState,
    owner_db: Option<PathBuf>,
    cache: PathBuf,
    load_s: &str,
) {
    *st.loaded_path.borrow_mut() = Some(cache);
    *st.loaded_target.borrow_mut() = Some(load_s.to_string());
    *st.preview_owner_db.borrow_mut() = owner_db;
}

/// Starts the frame pump for a seek on the auxiliary player.
fn pump_preview(
    st: &Rc<SeekPreviewState>,
    run_id: u64,
    load_s: &str,
    content_dur: f64,
    t: f64,
    optical: bool,
) {
    start_preview_frame_pump(st, run_id, load_s, content_dur, t, optical);
}

/// Seek target shared by the warm and reload finish paths.
struct SeekTarget<'a> {
    load_s: &'a str,
    content_dur: f64,
    t: f64,
    optical: bool,
}

/// Seeks an already-loaded warm player directly when possible; returns `Some(t)` — the
/// effective seek second — when a frame pump must finish the job instead.
fn warm_seek_or_pump_time(
    st: &SeekPreviewState,
    pr: &MpvPreviewGl,
    tgt: &SeekTarget<'_>,
    instant: bool,
    vo_ready: bool,
) -> Option<f64> {
    set_preview_tracks(&pr.mpv);
    let t = cap_preview_seek_time(tgt.t, tgt.content_dur);
    if instant && vo_ready && preview_run_seek(&pr.mpv, tgt.load_s, t, tgt.optical) {
        crate::preview_debug::info(format!(
            "do_seek warm instant seek={t:.2} ({})",
            crate::preview_debug::mpv_line(&pr.mpv)
        ));
        st.gl.queue_render();
        return None;
    }
    Some(t)
}

/// Warm path: render straight from the loaded clip when possible, else keep pumping.
fn warm_finish(
    st: &Rc<SeekPreviewState>,
    tgt: SeekTarget<'_>,
    instant: bool,
    vo_ready: bool,
    run_id: u64,
) {
    let mut g = st.preview.borrow_mut();
    let Some(pr) = g.as_mut() else { return };
    let Some(t) = warm_seek_or_pump_time(st, pr, &tgt, instant, vo_ready) else {
        return;
    };
    drop(g);
    pump_preview(st, run_id, tgt.load_s, tgt.content_dur, t, tgt.optical);
}

/// Reload path: reuse an in-flight load's pump or load the file fresh, then pump.
fn reload_finish(
    st: &Rc<SeekPreviewState>,
    owner_db: Option<PathBuf>,
    load_s: &str,
    content_dur: f64,
    t: f64,
    optical: bool,
    run_id: u64,
) {
    let mut g = st.preview.borrow_mut();
    let Some(pr) = g.as_mut() else { return };
    if preview_load_in_flight(st, load_s) {
        crate::preview_debug::info(format!(
            "do_seek load in flight, pump only seek={t:.2} ({load_s})"
        ));
        drop(g);
        pump_preview(st, run_id, load_s, content_dur, t, optical);
        return;
    }
    if !load_into_preview(st, pr, load_s) {
        return;
    }
    store_loaded_state(st, owner_db, preview_cache_path(load_s), load_s);
    drop(g);
    pump_preview(st, run_id, load_s, content_dur, t, optical);
}
