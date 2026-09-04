// Event-poll wait for a probed `.vob` duration (included from `session.rs`).

use libmpv2::events::Event;
use libmpv2::mpv_end_file_reason;

fn read_duration(m: &Mpv) -> Option<f64> {
    read_raw_duration(m).filter(|d| valid_duration(*d))
}

fn read_raw_duration(m: &Mpv) -> Option<f64> {
    m.get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
}

/// Valid duration once available; `Some(None)` = settled invalid (over-long VOB).
fn vob_duration_snapshot(m: &mut Mpv) -> Option<Option<f64>> {
    if let Some(d) = read_duration(m) {
        return Some(Some(d));
    }
    if read_raw_duration(m).is_some_and(|d| d > MAX_VOB_DUR_SEC) {
        return Some(None);
    }
    None
}

/// One event-poll step of [wait_vob_duration].
fn event_settled_duration(m: &mut Mpv) -> Option<Option<f64>> {
    match m.wait_event(0.05) {
        Some(Ok(Event::FileLoaded)) => vob_duration_snapshot(m),
        Some(Ok(Event::EndFile(r))) => {
            if r == mpv_end_file_reason::Error {
                Some(None)
            } else {
                Some(read_duration(m))
            }
        }
        Some(Err(_)) => {
            drain_events(m);
            None
        }
        Some(Ok(_)) | None => None,
    }
}

fn wait_vob_duration(m: &mut Mpv, deadline: Instant) -> Option<f64> {
    loop {
        if let Some(settled) = vob_duration_snapshot(m) {
            return settled;
        }
        if Instant::now() >= deadline {
            return None;
        }
        if let Some(settled) = event_settled_duration(m) {
            return settled;
        }
    }
}
