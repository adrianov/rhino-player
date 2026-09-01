// Engine-held-pause liveness tracking for the multi-monitor blackout.
//
// A "tech hold" marks an mpv pause that the app itself initiated for a brief engine operation
// (smooth `vf` swap, seek burst, chapter scrub) so blackout covers stay up — a user pause would
// clear them (`BlackoutSync` consults `tech_hold_active`). Include!'d from `screen_blackout.rs`;
// same module scope, so it shares the parent's imports.

thread_local! {
    /// Engine-held pauses (smooth `vf` swap, seek burst, chapter scrub) — not user pause.
    static TECH_HOLD: Cell<TechHold> = const { Cell::new(TechHold::idle()) };
    /// One bounded follow-up sync while a hold is outstanding (see [arm_hold_expiry_watch]).
    static HOLD_WATCH: Cell<bool> = const { Cell::new(false) };
}

/// Keep blackout up across an engine-held pause. Entering and leaving the hold changes what
/// [tech_hold_active] reports, so both edges refresh — a paused engine hold and a user pause look
/// the same to the transport, and no pause event follows a hold that ends while playback stays paused.
///
/// Holds are **liveness-bounded**: a leaked `begin` without its paired `end` (rapid-seek / filter-
/// rebuild interleavings) would otherwise pin `tech_hold_active` forever and silently disable
/// "user pause clears blackout" for the rest of the session. Every real hold is short — each site
/// releases within its own tail timer (~[SEEK_BURST_TAIL_IDLE_MS]-class delays, see
/// `app::seek_wiring` / `video_pref::smooth_off_playhead_refresh`) — so [HOLD_LIVENESS] far exceeds
/// any legitimate hold. An overdue hold stops reporting active, and a bounded watch re-syncs so the
/// covers actually drop.
const HOLD_LIVENESS: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Clone, Copy)]
struct TechHold {
    depth: u32,
    /// When the most recent `begin` ran; refreshed on every nested begin.
    since: Option<std::time::Instant>,
}

impl TechHold {
    const fn idle() -> Self {
        Self {
            depth: 0,
            since: None,
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn is_live(&self, now: std::time::Instant) -> bool {
        self.depth > 0
            && self
                .since
                .is_some_and(|since| now.duration_since(since) <= HOLD_LIVENESS)
    }
}

pub fn begin_tech_hold() {
    let entered = TECH_HOLD.with(|d| {
        let mut h = d.get();
        h.depth = h.depth.saturating_add(1);
        // Refresh liveness on every begin: a long burst of re-entries must not look stale while
        // its tail is still pending.
        h.since = Some(std::time::Instant::now());
        let entered = h.depth == 1;
        d.set(h);
        entered
    });
    if entered {
        refresh_for_hold();
        arm_hold_expiry_watch();
    }
}

/// Pair with [begin_tech_hold] when that hold ends.
pub fn end_tech_hold() {
    let left = TECH_HOLD.with(|d| {
        let mut h = d.get();
        h.depth = h.depth.saturating_sub(1);
        if h.depth == 0 {
            h.since = None;
        }
        let left = h.depth == 0;
        d.set(h);
        left
    });
    if left {
        refresh_for_hold();
    }
}

/// While a hold is outstanding, one follow-up sync at [HOLD_LIVENESS] — repeated only while the
/// hold has not drained. This is a bounded chain tied to the `begin` event (a leaked hold reports
/// inactive once overdue, and this pass is what drops the covers); with healthy pairing it fires
/// at most once per hold and always finds depth 0.
fn arm_hold_expiry_watch() {
    if HOLD_WATCH.replace(true) {
        return;
    }
    let _ = glib::timeout_add_local_once(HOLD_LIVENESS + std::time::Duration::from_secs(1), || {
        HOLD_WATCH.set(false);
        if TECH_HOLD.with(Cell::get).depth > 0 {
            refresh_for_hold();
            arm_hold_expiry_watch();
        }
    });
}

#[cfg(target_os = "macos")]
fn tech_hold_active() -> bool {
    TECH_HOLD.with(|d| d.get().is_live(std::time::Instant::now()))
}

/// Depth / liveness / hold age for the always-on cover decision log.
#[cfg(target_os = "macos")]
fn tech_hold_diag() -> (u32, bool, Option<f32>) {
    TECH_HOLD.with(|d| {
        let h = d.get();
        let now = std::time::Instant::now();
        (
            h.depth,
            h.is_live(now),
            h.since.map(|since| now.duration_since(since).as_secs_f32()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_without_begin_is_never_live() {
        assert!(!TechHold::idle().is_live(std::time::Instant::now()));
    }

    #[test]
    fn fresh_hold_is_live() {
        let mut h = TechHold::idle();
        h.depth = 1;
        h.since = Some(std::time::Instant::now());
        assert!(h.is_live(std::time::Instant::now()));
    }

    #[test]
    fn leaked_hold_expires() {
        let mut h = TechHold::idle();
        h.depth = 1;
        h.since =
            Some(std::time::Instant::now() - HOLD_LIVENESS - std::time::Duration::from_secs(1));
        assert!(!h.is_live(std::time::Instant::now()));
    }

    #[test]
    fn end_drains_depth_to_idle_shape() {
        let mut h = TechHold::idle();
        h.depth = h.depth.saturating_add(1);
        h.since = Some(std::time::Instant::now());
        h.depth = h.depth.saturating_sub(1);
        if h.depth == 0 {
            h.since = None;
        }
        assert_eq!((h.depth, h.since.is_none()), (0, true));
    }
}
