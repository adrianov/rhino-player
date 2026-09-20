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
    // The metadata is measured in the chain-output space of this moment; the
    // sizes must be captured beside it — the teardown below can change the
    // chain before the verdict maps the rect, and a stale mapping would cache
    // a filtered-space crop as decoded-space coordinates.
    let sizes = chain_sizes(mpv);
    remove_cropdetect(mpv);
    restore_hwdec(mpv, hw_backup);
    settle_after_teardown(probe, mpv);
    match (meta, sizes) {
        (Some(m), Some(s)) => meta_verdict(m, s, saw_deint),
        (Some(_), None) => {
            eprintln!("[rhino] bars: probe dropped: frame sizes unavailable beside cropdetect metadata");
            ProbeOutcome::NoData
        }
        (None, _) => ProbeOutcome::NoData,
    }
}

/// Convert cropdetect metadata into the probe verdict using the size pair
/// captured beside it: a meaningful crop (normalized to decoded-frame space), a
/// probed clean frame, or `NoData` for garbage metadata — the caller retries
/// instead of caching.
fn meta_verdict(meta: CropMeta, sizes: ChainSizes, saw_deint: bool) -> ProbeOutcome {
    match crop_from_meta(meta, sizes.vo) {
        Some(rect) => {
            let rect = scale_rect_between(rect, sizes.vo, sizes.decode);
            eprintln!(
                "[rhino] bars: detected crop={}x{}+{}+{}",
                rect.w, rect.h, rect.x, rect.y
            );
            ProbeOutcome::Final(BarState::Crop(rect), saw_deint)
        }
        // Metadata read, but no meaningful crop within the frame: probed clean.
        None if crop_meta_in_frame(sizes.vo.0, sizes.vo.1, meta) => {
            ProbeOutcome::Final(BarState::Clean, saw_deint)
        }
        None => ProbeOutcome::NoData,
    }
}
