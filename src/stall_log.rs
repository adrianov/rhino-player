//! Playback stall log: app-driven pauses around Smooth `vf` swaps (A/V resync), mpv cache
//! buffering, and mpv's own A/V desync warnings. Each event goes to stderr (`[rhino] stall:`) and
//! to the system log through `syslog(3)` — journald on Linux, the unified log on macOS — so stalls
//! can be audited after the session. Holds list the transport events that preceded them, so the
//! trigger of an unnecessary pause can be identified.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Transport events older than this are not listed as context for a new stall.
const TRIGGER_WINDOW: Duration = Duration::from_secs(3);
const TRIGGER_CAP: usize = 8;

thread_local! {
    static TRIGGERS: RefCell<VecDeque<(Instant, String)>> = const { RefCell::new(VecDeque::new()) };
    static OPEN_HOLD: Cell<Option<Hold>> = const { Cell::new(None) };
    static BUFFERING_SINCE: Cell<Option<Instant>> = const { Cell::new(None) };
    /// Cause of the next [Hold::begin]. Callers set this so a shared pause path can name
    /// budget shrink, cadence, speed, and the other stalls apart.
    static ARMED_REASON: Cell<Option<&'static str>> = const { Cell::new(None) };
}

/// One app-driven pause; [Hold::end] logs how long playback stayed paused.
#[derive(Clone, Copy)]
pub(crate) struct Hold {
    reason: &'static str,
    since: Instant,
}

/// Name the next pause. Overwrites a reason already armed for this stall.
pub(crate) fn set_reason(reason: &'static str) {
    ARMED_REASON.set(Some(reason));
}

/// Name the next pause only when nothing more specific is armed yet.
pub(crate) fn set_reason_if_empty(reason: &'static str) {
    ARMED_REASON.with(|c| {
        if c.get().is_none() {
            c.set(Some(reason));
        }
    });
}

/// Drop an armed reason that never became a pause (apply was a no-op).
pub(crate) fn clear_reason() {
    ARMED_REASON.set(None);
}

/// Arm [reason] for pauses inside [body], then drop it so a later stall cannot inherit the name.
pub(crate) fn with_reason<T>(reason: &'static str, body: impl FnOnce() -> T) -> T {
    set_reason(reason);
    let out = body();
    clear_reason();
    out
}

fn take_reason(fallback: &'static str) -> &'static str {
    ARMED_REASON.take().unwrap_or(fallback)
}

/// Stall that removes or restores Smooth without the pause/resume pair (external-load strip).
pub(crate) fn note(reason: &'static str) {
    emit(&format!("reason={reason}"));
}

impl Hold {
    pub(crate) fn begin(reason: &'static str, mpv: &libmpv2::Mpv) -> Self {
        let reason = take_reason(reason);
        if let Some(prev) = OPEN_HOLD.get() {
            emit(&format!(
                "hold {} never resumed ({:.2}s ago), superseded by {reason}",
                prev.reason,
                prev.since.elapsed().as_secs_f64()
            ));
        }
        let hold = Self {
            reason,
            since: Instant::now(),
        };
        OPEN_HOLD.set(Some(hold));
        emit(&format!(
            "pause hold reason={reason} {} after=[{}]",
            mpv_snapshot(mpv),
            recent_triggers()
        ));
        hold
    }

    pub(crate) fn end(self) {
        if OPEN_HOLD.get().is_some_and(|h| h.since == self.since) {
            OPEN_HOLD.set(None);
        }
        emit(&format!(
            "resume reason={} held={:.2}s",
            self.reason,
            self.since.elapsed().as_secs_f64()
        ));
    }
}

/// Remember a transport event (mpv property change, seek, reconfig) as context for later stalls.
pub(crate) fn trigger(what: impl Into<String>) {
    TRIGGERS.with(|t| {
        let mut t = t.borrow_mut();
        if t.len() == TRIGGER_CAP {
            t.pop_front();
        }
        t.push_back((Instant::now(), what.into()));
    });
}

/// mpv `paused-for-cache` flip: playback waits for the demuxer cache.
pub(crate) fn buffering(on: bool) {
    match (on, BUFFERING_SINCE.get()) {
        (true, None) => {
            BUFFERING_SINCE.set(Some(Instant::now()));
            emit(&format!("mpv buffering after=[{}]", recent_triggers()));
        }
        (false, Some(since)) => {
            BUFFERING_SINCE.set(None);
            emit(&format!(
                "mpv buffering done held={:.2}s",
                since.elapsed().as_secs_f64()
            ));
        }
        _ => {}
    }
}

/// Logs mpv's A/V desync warning as a stall; returns whether the message was consumed.
pub(crate) fn mpv_warning(prefix: &str, text: &str) -> bool {
    if !text.contains("desynchronisation") {
        return false;
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    emit(&format!(
        "mpv [{prefix}] {text} after=[{}]",
        recent_triggers()
    ));
    true
}

fn mpv_snapshot(mpv: &libmpv2::Mpv) -> String {
    let num = |name: &str| crate::video_pref::fmt_secs_opt(mpv.get_property::<f64>(name).ok(), 2);
    format!(
        "t={} speed={} container-fps={} vf-fps={} a-v={} load-hold={} file={}",
        num("time-pos"),
        num("speed"),
        num("container-fps"),
        num("estimated-vf-fps"),
        num("avsync"),
        crate::video_pref::smooth_load_hold_active(),
        mpv.get_property::<String>("filename").unwrap_or_default(),
    )
}

fn recent_triggers() -> String {
    TRIGGERS.with(|t| {
        t.borrow()
            .iter()
            .filter(|(at, _)| at.elapsed() <= TRIGGER_WINDOW)
            .map(|(at, what)| format!("{what} -{:.2}s", at.elapsed().as_secs_f64()))
            .collect::<Vec<_>>()
            .join(", ")
    })
}

fn emit(msg: &str) {
    eprintln!("[rhino] stall: {msg}");
    static OPEN: std::sync::Once = std::sync::Once::new();
    OPEN.call_once(|| unsafe {
        libc::openlog(
            b"rhino-player\0".as_ptr().cast(),
            libc::LOG_PID,
            libc::LOG_USER,
        );
    });
    let Ok(line) = std::ffi::CString::new(format!("stall: {msg}")) else {
        return;
    };
    unsafe { libc::syslog(libc::LOG_NOTICE, b"%s\0".as_ptr().cast(), line.as_ptr()) };
}

#[cfg(test)]
mod reason_tests {
    use super::{clear_reason, set_reason, set_reason_if_empty, take_reason, with_reason};

    #[test]
    fn armed_reason_wins_over_fallback() {
        clear_reason();
        set_reason("smooth budget shrink");
        assert_eq!(take_reason("smooth graph replace"), "smooth budget shrink");
        assert_eq!(take_reason("smooth graph replace"), "smooth graph replace");
    }

    #[test]
    fn specific_reason_is_kept() {
        clear_reason();
        set_reason("playback speed");
        set_reason_if_empty("smooth cadence");
        assert_eq!(take_reason("smooth strip"), "playback speed");
    }

    #[test]
    fn with_reason_does_not_outlive_the_call() {
        clear_reason();
        with_reason("smooth budget shrink", || {
            assert_eq!(take_reason("smooth resync"), "smooth budget shrink");
        });
        assert_eq!(take_reason("smooth resync"), "smooth resync");
    }
}
