// Stable-poll capture loop for `screenshot-raw` (include!'d from `thumb_screenshot_raw.rs`):
// bright frames accept immediately; dark/flat frames need a run of stable polls.

/// One encoded screenshot; `dark` marks a mostly-black frame (real dark scene or not-yet-decoded VO buffer).
/// `flat` marks an almost-uniform buffer (mpv placeholder after seek before decode finishes).
struct Capture {
    webp: Vec<u8>,
    dark: bool,
    flat: bool,
}

/// Consecutive dark polls (50 ms apart) before a dark frame counts as the real decoded picture
/// (legit dark scene at the continue position) rather than an undecoded buffer.
const DARK_STABLE_POLLS: u32 = 20;

/// Same stability window for flat placeholder frames after hr-seek.
const FLAT_STABLE_POLLS: u32 = 20;

/// Stable-run trackers for the screenshot poll loop.
struct PollState {
    polls: u32,
    dark_run: u32,
    flat_run: u32,
    dark_webp: Option<Vec<u8>>,
    flat_webp: Option<Vec<u8>>,
}

impl PollState {
    /// A failed raw capture resets both stable runs.
    fn reset_runs(&mut self) {
        self.dark_run = 0;
        self.flat_run = 0;
    }

    /// Fold one raw capture into the trackers; `Some` when a bright detailed frame accepts now.
    /// Flat (almost-uniform) is checked before dark so solid color boards nudge instead of
    /// waiting out the dark-stability path.
    fn fold_capture(&mut self, c: Capture) -> Option<Vec<u8>> {
        if c.flat {
            self.flat_run += 1;
            self.dark_run = 0;
            self.flat_webp = Some(c.webp);
            return None;
        }
        if c.dark {
            self.dark_run += 1;
            self.flat_run = 0;
            self.dark_webp = Some(c.webp);
            return None;
        }
        Some(c.webp)
    }

    /// Frame accepted because its dark/flat run reached its stability window.
    fn stable_accept(&self) -> Option<Vec<u8>> {
        if self.dark_run >= DARK_STABLE_POLLS {
            eprintln!(
                "[rhino] grid_thumb dark frame accepted after {} stable polls{}",
                self.dark_run,
                thumb_src_suffix()
            );
            return self.dark_webp.clone();
        }
        if self.flat_run >= FLAT_STABLE_POLLS {
            eprintln!(
                "[rhino] grid_thumb flat frame accepted after {} stable polls{}",
                self.flat_run,
                thumb_src_suffix()
            );
            return self.flat_webp.clone();
        }
        None
    }
}

enum PollOutcome {
    Accept(Vec<u8>),
    Continue,
    Timeout,
}

/// One poll step: drain events, frame-step, fold the raw capture, then judge stability/deadline.
fn poll_once(m: &mut Mpv, st: &mut PollState, deadline: Instant) -> PollOutcome {
    let _ = m.command("frame-step", &[] as &[&str]);
    match try_screenshot_raw_webp(m, st.polls == 0) {
        Some(c) => {
            if let Some(webp) = st.fold_capture(c) {
                return PollOutcome::Accept(webp);
            }
        }
        None => st.reset_runs(),
    }
    if let Some(webp) = st.stable_accept() {
        return PollOutcome::Accept(webp);
    }
    st.polls += 1;
    if Instant::now() >= deadline {
        return match timeout_poll_accept(st.polls, st.dark_webp.take().or(st.flat_webp.take())) {
            Some(webp) => PollOutcome::Accept(webp),
            None => PollOutcome::Timeout,
        };
    }
    PollOutcome::Continue
}

/// Deadline hit: keep a previously captured dark or flat frame, else give up.
fn timeout_poll_accept(polls: u32, blank: Option<Vec<u8>>) -> Option<Vec<u8>> {
    if blank.is_some() {
        eprintln!(
            "[rhino] grid_thumb blank frame accepted at timeout{}",
            thumb_src_suffix()
        );
        return blank;
    }
    eprintln!(
        "[rhino] grid_thumb screenshot-raw capture timeout after {polls} polls{}",
        thumb_src_suffix()
    );
    None
}

/// Poll until one decoded frame is available, then return WebP bytes (no temp files).
/// A bright detailed frame returns immediately; stable dark or flat frames are accepted after
/// their stability windows. Callers may nudge the seek when the result is still almost uniform.
pub(super) fn capture_screenshot_webp(m: &mut Mpv, wait_secs: u64) -> Option<Vec<u8>> {
    let mut st = PollState {
        polls: 0,
        dark_run: 0,
        flat_run: 0,
        dark_webp: None,
        flat_webp: None,
    };
    let deadline = Instant::now() + Duration::from_secs(wait_secs);
    loop {
        while m.wait_event(0.0).is_some() {}
        match poll_once(m, &mut st, deadline) {
            PollOutcome::Accept(webp) => return Some(webp),
            PollOutcome::Timeout => return None,
            PollOutcome::Continue => {}
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
