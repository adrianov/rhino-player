// Gather completion: timer fires, verdict is read, the probe tears itself down;
// deciding final vs retry belongs to the probe_defer rescheduling chains.

fn finish_cropdetect(
    player: &Player,
    probe: &Rc<BarProbe>,
    gen: u64,
    hw_backup: Option<String>,
    on_done: Rc<dyn Fn()>,
) {
    if probe.gen.get() != gen {
        abort_stale_probe(player, probe, hw_backup.as_deref());
        return;
    }
    probe.gathering.set(false);
    match take_probe_result(player, probe, hw_backup.as_deref()) {
        ProbeOutcome::Final(state, saw_deint) => {
            probe.saw_deint.set(saw_deint);
            probe.state.set(state);
            on_done();
        }
        ProbeOutcome::NoData => defer_no_data(player, probe, gen, on_done, "no cropdetect metadata"),
    }
}

fn abort_stale_probe(player: &Player, probe: &Rc<BarProbe>, hw_backup: Option<&str>) {
    if let Some(b) = player.borrow().as_ref() {
        remove_cropdetect(&b.mpv);
        restore_hwdec(&b.mpv, hw_backup);
        settle_after_teardown(probe, &b.mpv);
    }
}

/// After a probe's own vf/hwdec teardown, record the settled chain: a `VideoReconfig`
/// still matching it is probe-generated (its own cleanup), never an external change.
fn settle_after_teardown(probe: &Rc<BarProbe>, mpv: &Mpv) {
    probe.settle_cleanup_vf(mpv.get_property::<String>("vf").unwrap_or_default());
}

/// Gather verdict: real cropdetect metadata read (final, persisted) vs nothing read.
enum ProbeOutcome {
    Final(BarState, bool),
    /// No metadata in the gather window (paused start, vf rebuild, slow decode).
    NoData,
}

fn take_probe_result(
    player: &Player,
    probe: &Rc<BarProbe>,
    hw_backup: Option<&str>,
) -> ProbeOutcome {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return ProbeOutcome::NoData;
    };
    let mpv = &b.mpv;
    let saw_deint = crate::video_pref::bob_deinterlace_in_vf(
        &mpv.get_property::<String>("vf").unwrap_or_default(),
    );
    let meta = read_cropdetect_meta(mpv);
    remove_cropdetect(mpv);
    restore_hwdec(mpv, hw_backup);
    settle_after_teardown(probe, mpv);
    match meta {
        None => ProbeOutcome::NoData,
        Some(m) => meta_verdict(mpv, m, saw_deint),
    }
}

/// Convert cropdetect metadata into the probe verdict: a meaningful crop, a
/// probed clean frame, or `NoData` when the metadata is unusable (garbage /
/// frame size unreadable) — the caller retries instead of caching.
fn meta_verdict(mpv: &Mpv, meta: CropMeta, saw_deint: bool) -> ProbeOutcome {
    match crop_from_meta(mpv, meta) {
        Some(rect) => {
            eprintln!(
                "[rhino] bars: detected crop={}x{}+{}+{}",
                rect.w, rect.h, rect.x, rect.y
            );
            ProbeOutcome::Final(BarState::Crop(rect), saw_deint)
        }
        // Metadata read, but no meaningful crop within the frame: probed clean.
        None if meta_in_probed_frame(mpv, meta) => ProbeOutcome::Final(BarState::Clean, saw_deint),
        None => ProbeOutcome::NoData,
    }
}

/// `true` when the frame size is readable and the metadata lies within the frame
/// (so a `crop_from_meta` miss really was "no meaningful strips", not garbage).
fn meta_in_probed_frame(mpv: &Mpv, meta: CropMeta) -> bool {
    let width = mpv.get_property::<i64>("width").unwrap_or(0);
    let height = mpv.get_property::<i64>("height").unwrap_or(0);
    width > 0 && height > 0 && crop_meta_in_frame(width, height, meta)
}
