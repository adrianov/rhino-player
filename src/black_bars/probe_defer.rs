// Probe rescheduling: bounded follow-up chains when cropdetect cannot run yet
// (decode size missing, paused start, vf rebuild mid-gather). Never a "clean"
// verdict — `Pending` until real metadata is read or the budget runs out.
const READY_RETRY: Duration = Duration::from_millis(250);
const READY_RETRY_MAX: u8 = 8;
/// No-frame-data retries before the probe settles as inconclusive (`Pending`).
const BAR_META_RETRIES: u8 = 4;
/// Gap between no-frame-data retries (paused start / vf rebuild mid-gather).
const META_RETRY: Duration = Duration::from_millis(2000);

/// Bounded follow-ups after detect delay when width/height are still unset.
fn defer_until_video_ready(
    player: &Player,
    probe: &Rc<BarProbe>,
    gen: u64,
    on_done: Rc<dyn Fn()>,
) {
    let left = probe.ready_left.get();
    if left == 0 {
        // Video size never arrived; not a "clean" verdict — decode start reconfig re-arms.
        eprintln!("[rhino] bars: probe waiting (video never ready); re-arms on reconfig");
        return;
    }
    probe.ready_left.set(left - 1);
    eprintln!(
        "[rhino] bars: probe deferred (no video yet), retry {} left",
        left - 1
    );
    let player = Rc::clone(player);
    let probe = Rc::clone(probe);
    glib::timeout_add_local_once(READY_RETRY, move || {
        if probe.gen.get() != gen {
            return;
        }
        begin_cropdetect(&player, &probe, gen, on_done);
    });
}

/// A gather without usable metadata (or a paused start) is never cached as clean:
/// stay `Pending` and retry on a short bounded chain; event pumps re-arm afterwards.
fn defer_no_data(
    player: &Player,
    probe: &Rc<BarProbe>,
    gen: u64,
    on_done: Rc<dyn Fn()>,
    why: &str,
) {
    probe.state.set(BarState::Pending);
    if !probe.take_meta_retry() {
        eprintln!("[rhino] bars: probe inconclusive ({why}); re-arms on reconfig / unpause");
        return;
    }
    eprintln!("[rhino] bars: {why}; retrying");
    let player = Rc::clone(player);
    let probe = Rc::clone(probe);
    glib::timeout_add_local_once(META_RETRY, move || {
        if probe.gen.get() != gen {
            return;
        }
        begin_cropdetect(&player, &probe, gen, on_done);
    });
}

fn probe_paused(player: &Player) -> bool {
    player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.mpv.get_property::<bool>("pause").unwrap_or(false))
}
