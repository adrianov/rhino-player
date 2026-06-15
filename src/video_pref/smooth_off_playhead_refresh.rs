/// Same tail delay as arrow-key seeks ([app::seek_wiring::SEEK_BURST_TAIL_IDLE_MS]).
const VF_SWAP_TAIL_MS: u64 = 1000;

thread_local! {
    /// Set before [crate::app::request_smooth_60_transport_resync] from the vf-swap tail so
    /// [apply_mpv_video] runs **`vf add`** only (env was prepared on the defer leg).
    static VF_SWAP_POST_SEEK_ATTACH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static VF_SWAP_GEN: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static VF_SWAP_DEFER_IN_FLIGHT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static VF_SWAP_REQUEST_SMOOTH_RESYNC: std::cell::RefCell<Option<Rc<dyn Fn()>>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) fn register_vf_swap_smooth_resync(f: Rc<dyn Fn()>) {
    VF_SWAP_REQUEST_SMOOTH_RESYNC.with(|slot| {
        *slot.borrow_mut() = Some(f);
    });
}

pub(crate) fn vf_swap_post_seek_attach_active() -> bool {
    VF_SWAP_POST_SEEK_ATTACH.get()
}

pub(crate) fn vf_swap_clear_post_seek_attach() {
    VF_SWAP_POST_SEEK_ATTACH.set(false);
}

pub(crate) fn vf_swap_defer_in_flight() -> bool {
    VF_SWAP_DEFER_IN_FLIGHT.get()
}

/// Drop a pending **`defer_smooth_vf_swap`** tail (e.g. user turned Smooth on again).
pub(crate) fn cancel_deferred_vf_swap() {
    VF_SWAP_GEN.set(VF_SWAP_GEN.get().wrapping_add(1));
    VF_SWAP_DEFER_IN_FLIGHT.set(false);
    VF_SWAP_POST_SEEK_ATTACH.set(false);
}

fn request_smooth_resync_after_swap() {
    VF_SWAP_REQUEST_SMOOTH_RESYNC.with(|slot| {
        if let Some(f) = slot.borrow().as_ref() {
            f();
        }
    });
}

fn vf_swap_keyframe_seek(mpv: &Mpv, bundle: Option<&MpvBundle>, tag: &str) {
    let _ = unload_smooth_on_pause(mpv, bundle);
    let Some(t) = vf_resync_playhead_sec(mpv, bundle) else {
        eprintln!("[rhino] video: {tag} keyframe resync skipped (no playhead)");
        return;
    };
    let s = format!("{t:.4}");
    let _ = mpv.command("seek", &[s.as_str(), "absolute+keyframes"]);
    eprintln!("[rhino] video: {tag} keyframe resync seek t={s}");
}

fn schedule_vf_swap_tail(player: &Rc<RefCell<Option<MpvBundle>>>, snap: VfAvSnap, reattach_smooth: bool) {
    let gen = VF_SWAP_GEN.get().saturating_add(1);
    VF_SWAP_GEN.set(gen);
    let p = Rc::clone(player);
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(VF_SWAP_TAIL_MS), move || {
        VF_SWAP_DEFER_IN_FLIGHT.set(false);
        if VF_SWAP_GEN.get() != gen {
            return;
        }
        if reattach_smooth {
            VF_SWAP_POST_SEEK_ATTACH.set(true);
            request_smooth_resync_after_swap();
        }
        let g = p.borrow();
        let Some(b) = g.as_ref() else {
            return;
        };
        vf_swap_unpause(&b.mpv, &snap);
        vf_av_ping_render(Some(b));
        log_smooth_avsync(&b.mpv);
    });
}

/// Strip **`vf`**, keyframe-seek, then debounced **`vf add`** — only when replacing an existing graph.
pub(crate) fn defer_smooth_vf_swap(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    snap: VfAvSnap,
    reattach_smooth: bool,
    tag: &str,
) {
    if VF_SWAP_DEFER_IN_FLIGHT.get() {
        eprintln!("[rhino] video: {tag} defer skipped (swap already in flight)");
        return;
    }
    VF_SWAP_DEFER_IN_FLIGHT.set(true);
    vf_swap_keyframe_seek(mpv, bundle, tag);
    schedule_vf_swap_tail(player, snap, reattach_smooth);
}

pub(crate) fn smooth_off_refresh_playhead(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    snap: &VfAvSnap,
) {
    let Some(t) = vf_resync_playhead_sec(mpv, bundle) else {
        eprintln!("[rhino] video: smooth-off keyframe resync skipped (no playhead)");
        vf_swap_unpause(mpv, snap);
        vf_av_ping_render(bundle);
        return;
    };
    let s = format!("{t:.4}");
    let _ = mpv.command("seek", &[s.as_str(), "absolute+keyframes"]);
    eprintln!("[rhino] video: smooth-off keyframe resync seek t={s}");
    vf_swap_unpause(mpv, snap);
    vf_av_ping_render(bundle);
}

/// Pause + playhead captured **before** `aid` changes (decoder reopen would skew clocks).
pub(crate) struct AudioTrackAvSnap {
    snap: VfAvSnap,
    playhead: f64,
}

/// Call immediately before `set_property("aid", …)` when Smooth **`vf`** is active and playing.
pub(crate) fn snap_audio_track_av_resync(b: &crate::mpv_embed::MpvBundle) -> Option<AudioTrackAvSnap> {
    let mpv = &b.mpv;
    if !vf_chain_has_vapoursynth(mpv) || mpv.get_property::<bool>("pause").unwrap_or(true) {
        return None;
    }
    cancel_deferred_vf_swap();
    let playhead = vf_resync_playhead_sec(mpv, Some(b))?;
    Some(AudioTrackAvSnap {
        snap: vf_swap_snap(mpv, true),
        playhead,
    })
}

fn schedule_audio_track_resync_tail() {
    let gen = VF_SWAP_GEN.get().saturating_add(1);
    VF_SWAP_GEN.set(gen);
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(VF_SWAP_TAIL_MS), move || {
        if VF_SWAP_GEN.get() != gen {
            return;
        }
        request_smooth_resync_after_swap();
    });
}

/// Exact seek to the pre-**`aid`** playhead, then debounced transport resync after the **`vf`** re-inits.
pub(crate) fn finish_audio_track_av_resync(
    b: &crate::mpv_embed::MpvBundle,
    prep: Option<AudioTrackAvSnap>,
) {
    let Some(prep) = prep else {
        return;
    };
    let mpv = &b.mpv;
    let s = format!("{:.4}", prep.playhead);
    let _ = mpv.command("seek", &[s.as_str(), "absolute+exact"]);
    eprintln!("[rhino] video: audio-track playhead resync seek t={s}");
    vf_swap_unpause(mpv, &prep.snap);
    vf_av_ping_render(Some(b));
    schedule_audio_track_resync_tail();
    log_smooth_avsync(mpv);
}
