// Gather completion: the last sample is in, the verdict is read; deciding final vs
// retry belongs to the probe_defer rescheduling chains.

fn finish_gather(run: &GatherRun, gather: Gather) {
    let probe = &run.probe;
    probe.gathering.set(false);
    match gather_verdict(gather) {
        ProbeOutcome::Final(state) => {
            probe.saw_deint.set(deint_in_vf(&run.player));
            probe.state.set(state);
            (run.on_done)();
        }
        ProbeOutcome::NoData => defer_no_data(
            &run.player,
            probe,
            run.gen,
            Rc::clone(&run.on_done),
            "no picture in sampled frames",
        ),
    }
}

fn deint_in_vf(player: &Player) -> bool {
    player.borrow().as_ref().is_some_and(|b| {
        crate::video_pref::bob_deinterlace_in_vf(&b.mpv.get_property::<String>("vf").unwrap_or_default())
    })
}

/// Gather verdict: a usable reading (final, persisted) vs nothing usable.
enum ProbeOutcome {
    Final(BarState),
    /// Only dark frames, no readable frame, or an implausible reading.
    NoData,
}

/// Convert the united reading into the probe verdict using the size pair captured
/// beside it: a meaningful crop (normalized to decoded-frame space), a probed clean
/// full frame, or `NoData` for implausible readings — the caller retries instead of caching.
fn gather_verdict(gather: Gather) -> ProbeOutcome {
    let (Some(sizes), Some(content)) = (gather.sizes, gather.content) else {
        return ProbeOutcome::NoData;
    };
    match classify_reading(content, sizes.vo) {
        ReadingVerdict::Crop(rect) => {
            let rect = scale_rect_between(rect, sizes.vo, sizes.decode);
            eprintln!(
                "[rhino] bars: detected crop={}x{}+{}+{}",
                rect.w, rect.h, rect.x, rect.y
            );
            ProbeOutcome::Final(BarState::Crop(rect))
        }
        ReadingVerdict::Clean => {
            eprintln!("[rhino] bars: probed clean (full-frame reading)");
            ProbeOutcome::Final(BarState::Clean)
        }
        ReadingVerdict::Garbage => {
            eprintln!(
                "[rhino] bars: probe rejected implausible reading {}x{}+{}+{} on {}x{}; retrying",
                content.w, content.h, content.x, content.y, sizes.vo.0, sizes.vo.1
            );
            ProbeOutcome::NoData
        }
    }
}
