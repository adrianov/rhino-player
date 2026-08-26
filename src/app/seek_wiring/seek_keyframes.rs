/// Idle after the last seek in a burst: unpause if playback was running before the burst, then reattach Smooth when due.
const SEEK_BURST_TAIL_IDLE_MS: u64 = 1000;

#[derive(Clone, Copy)]
enum SeekKeyframeKind {
    /// Pause-if-playing before seek; after idle, unpause only if the burst began while playing (arrow keys).
    ArrowBurst,
    /// Leave pause alone; debounce Smooth reattach when the seek starts while playing (seek bar, MPRIS).
    ScaleOrExternal,
}

struct SeekKeyframeParams<'a> {
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    gl: &'a gtk::GLArea,
    smooth_seek_debounce: &'a Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: &'a Rc<Cell<bool>>,
    play_toggle: &'a PlayToggleCtx,
    dvd_bar: Option<&'a Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>>,
}

fn cancel_smooth_seek_debounce(slot: &Rc<RefCell<Option<glib::SourceId>>>) {
    drop_glib_source(slot.as_ref());
}

fn schedule_smooth_vf_only_tail(slot: &Rc<RefCell<Option<glib::SourceId>>>, gl: gtk::GLArea) {
    cancel_smooth_seek_debounce(slot);
    let deb = Rc::clone(slot);
    let gl2 = gl.clone();
    let id =
        glib::timeout_add_local_once(Duration::from_millis(SEEK_BURST_TAIL_IDLE_MS), move || {
            *deb.borrow_mut() = None;
            request_smooth_60_transport_resync();
            gl2.queue_render();
        });
    *slot.borrow_mut() = Some(id);
}

fn schedule_seek_burst_tail(
    slot: &Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    gl: gtk::GLArea,
    play_toggle: PlayToggleCtx,
) {
    cancel_smooth_seek_debounce(slot);
    let deb = Rc::clone(slot);
    let gl2 = gl.clone();
    let id =
        glib::timeout_add_local_once(Duration::from_millis(SEEK_BURST_TAIL_IDLE_MS), move || {
            *deb.borrow_mut() = None;
            let trust_unpause = resume_after_seek_idle.replace(false);
            if trust_unpause {
                let _ = apply_mpv_pause(&play_toggle, false);
                crate::screen_blackout::end_tech_hold();
            }
            request_smooth_60_transport_resync();
            gl2.queue_render();
        });
    *slot.borrow_mut() = Some(id);
}

/// Seek main mpv with `absolute+keyframes`. Strip vapoursynth first when present — otherwise mpv
/// paints a black placeholder while the filter restarts. Playing seeks reattach Smooth after the
/// burst / scale idle (without pausing); paused seeks restore on unpause.
///
/// **[SeekKeyframeKind::ArrowBurst]**: pause through **`apply_mpv_pause`** when the clip was
/// playing; remember “should resume” for the whole burst; after [`SEEK_BURST_TAIL_IDLE_MS`] without
/// another seek, unpause if so — coalesces rapid arrow seeks.
///
/// **[SeekKeyframeKind::ScaleOrExternal]**: leaves pause alone; if this seek begins while playing
/// and Smooth was stripped, debounce Smooth resync only. If an arrow burst left
/// **`resume_after_seek_idle`** latched, the same tail timer still runs (seek-bar scrub while
/// “held” paused for arrows).
fn seek_keyframes_after_command(
    p: &SeekKeyframeParams<'_>,
    kind: SeekKeyframeKind,
    paused_before: bool,
    stripped_smooth: bool,
) {
    p.gl.queue_render();
    if p.resume_after_seek_idle.get() {
        schedule_seek_burst_tail(
            p.smooth_seek_debounce,
            p.resume_after_seek_idle.clone(),
            p.gl.clone(),
            p.play_toggle.clone(),
        );
    } else if matches!(kind, SeekKeyframeKind::ScaleOrExternal) && !paused_before && stripped_smooth
    {
        schedule_smooth_vf_only_tail(p.smooth_seek_debounce, p.gl.clone());
    }
}

fn try_dvd_global_seek(p: &SeekKeyframeParams<'_>, seconds: &str, resume_playing: bool) -> bool {
    let Ok(t) = seconds.parse::<f64>() else {
        return false;
    };
    if !t.is_finite() {
        return false;
    }
    let ok = crate::dvd_vob_timeline::seek_global(p.player, t, p.dvd_bar, resume_playing);
    if !ok {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "try_dvd_global_seek: seek_global returned false for t={t:.2} bar={}",
            if p.dvd_bar.is_some_and(|b| b.borrow().is_some()) {
                "cached"
            } else {
                "missing"
            }
        ));
    }
    ok
}

fn smooth_stripped_on_bundle(player: &Rc<RefCell<Option<MpvBundle>>>) -> bool {
    player
        .borrow()
        .as_ref()
        .is_some_and(MpvBundle::smooth_vf_stripped_this_open)
}

/// Keyframe seek inside the current file. Skipped for DVD VOB sets, which only seek globally.
/// Returns whether Smooth was stripped for this jump.
fn seek_local_file(b: &MpvBundle, seconds: &str) -> bool {
    let shell = b.me_budget_shell_path.borrow();
    if crate::media_probe::shell_media_path(&b.mpv, shell.as_deref())
        .is_some_and(|path| crate::video_ext::is_dvd_vob_path(&path))
    {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "seek blocked: DVD global seek failed for t={seconds}s (no local fallback)"
        ));
        return false;
    }
    let stripped = video_pref::unload_smooth_for_seek(&b.mpv, Some(b));
    if stripped {
        eprintln!("[rhino] video: vf stripped before seek");
    }
    let _ = b.mpv.command("seek", &[seconds, "absolute+keyframes"]);
    stripped
}

fn main_player_seek_keyframes(p: &SeekKeyframeParams<'_>, kind: SeekKeyframeKind, seconds: &str) {
    cancel_smooth_seek_debounce(p.smooth_seek_debounce);
    // Drop a pending post-seek Smooth rebuild so a rapid next seek cannot reattach mid-jump.
    cancel_smooth_60_transport_resync();
    let paused_before = match read_paused_before(p) {
        Some(paused) => paused,
        None => return,
    };
    if matches!(kind, SeekKeyframeKind::ArrowBurst) {
        latch_arrow_burst_pause(p, paused_before);
    }
    if try_dvd_global_seek(p, seconds, !paused_before) {
        seek_keyframes_after_command(p, kind, paused_before, smooth_stripped_on_bundle(p.player));
        return;
    }
    // A blocked seek still runs the tail below: it owns the arrow-burst unpause and the blackout
    // hold, so returning early here would leave playback paused and blackout stuck on.
    let stripped = seek_local_fallback(p, seconds);
    seek_keyframes_after_command(p, kind, paused_before, stripped);
}

fn read_paused_before(p: &SeekKeyframeParams<'_>) -> Option<bool> {
    let g = p.player.borrow();
    let b = g.as_ref()?;
    Some(b.mpv.get_property::<bool>("pause").unwrap_or(true))
}

/// Arrow burst while playing: latch "should resume" for the whole burst and hold paused.
fn latch_arrow_burst_pause(p: &SeekKeyframeParams<'_>, paused_before: bool) {
    let was_playing = !paused_before;
    p.resume_after_seek_idle
        .set(p.resume_after_seek_idle.get() || was_playing);
    if was_playing {
        crate::screen_blackout::begin_tech_hold();
        let _ = apply_mpv_pause(p.play_toggle, true);
    }
}

fn seek_local_fallback(p: &SeekKeyframeParams<'_>, seconds: &str) -> bool {
    p.player
        .borrow()
        .as_ref()
        .map(|b| seek_local_file(b, seconds))
        .unwrap_or(false)
}
