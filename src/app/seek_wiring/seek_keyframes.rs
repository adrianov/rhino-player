/// Idle after the last seek in a burst: unpause if playback was running before the burst, then reattach Smooth when due.
const SEEK_BURST_TAIL_IDLE_MS: u64 = 1000;

#[derive(Clone, Copy)]
enum SeekKeyframeKind {
    /// Pause-if-playing before seek; after idle, unpause only if the burst began while playing (arrow keys).
    ArrowBurst,
    /// Do not change pause state; debounce Smooth reattach when the seek starts while playing (seek bar, MPRIS).
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
    let id = glib::timeout_add_local_once(Duration::from_millis(SEEK_BURST_TAIL_IDLE_MS), move || {
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
    let id = glib::timeout_add_local_once(Duration::from_millis(SEEK_BURST_TAIL_IDLE_MS), move || {
        *deb.borrow_mut() = None;
        let trust_unpause = resume_after_seek_idle.replace(false);
        if trust_unpause {
            let _ = apply_mpv_pause(&play_toggle, false);
        }
        request_smooth_60_transport_resync();
        gl2.queue_render();
    });
    *slot.borrow_mut() = Some(id);
}

/// Seek main mpv with `absolute+keyframes`. Drops vapoursynth **`vf`** before the seek when still
/// present.
///
/// **[SeekKeyframeKind::ArrowBurst]**: pause through **`apply_mpv_pause`** when the clip was
/// playing; remember “should resume” for the whole burst; after [`SEEK_BURST_TAIL_IDLE_MS`] without
/// another seek, unpause if so and reattach Smooth — coalesces rapid arrow seeks.
///
/// **[SeekKeyframeKind::ScaleOrExternal]**: leaves pause alone; if this seek begins while playing,
/// debounce Smooth reattach only. If an arrow burst left **`resume_after_seek_idle`** latched, the
/// same tail timer still runs (seek-bar scrub while “held” paused for arrows).
fn seek_keyframes_after_command(
    p: &SeekKeyframeParams<'_>,
    kind: SeekKeyframeKind,
    paused_before: bool,
) {
    p.gl.queue_render();
    if p.resume_after_seek_idle.get() {
        schedule_seek_burst_tail(
            p.smooth_seek_debounce,
            p.resume_after_seek_idle.clone(),
            p.gl.clone(),
            p.play_toggle.clone(),
        );
    } else if matches!(kind, SeekKeyframeKind::ScaleOrExternal) && !paused_before {
        schedule_smooth_vf_only_tail(p.smooth_seek_debounce, p.gl.clone());
    }
}

fn try_dvd_global_seek(p: &SeekKeyframeParams<'_>, seconds: &str) -> bool {
    let Ok(t) = seconds.parse::<f64>() else {
        return false;
    };
    if !t.is_finite() {
        return false;
    }
    let ok = crate::dvd_vob_timeline::seek_global(p.player, t, p.dvd_bar);
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

fn main_player_seek_keyframes(p: &SeekKeyframeParams<'_>, kind: SeekKeyframeKind, seconds: &str) {
    cancel_smooth_seek_debounce(p.smooth_seek_debounce);
    let paused_before;
    {
        let g = p.player.borrow();
        let Some(b) = g.as_ref() else {
            return;
        };
        paused_before = b.mpv.get_property::<bool>("pause").unwrap_or(true);
    }
    if matches!(kind, SeekKeyframeKind::ArrowBurst) {
        let was_playing = !paused_before;
        p.resume_after_seek_idle
            .set(p.resume_after_seek_idle.get() || was_playing);
        if was_playing {
            let _ = apply_mpv_pause(p.play_toggle, true);
        }
    }
    if try_dvd_global_seek(p, seconds) {
        seek_keyframes_after_command(p, kind, paused_before);
        return;
    }
    {
        let g = p.player.borrow();
        let Some(b) = g.as_ref() else {
            return;
        };
        let _ = video_pref::unload_smooth_on_pause(&b.mpv);
        let _ = b.mpv.command("seek", &[seconds, "absolute+keyframes"]);
    }
    seek_keyframes_after_command(p, kind, paused_before);
}
