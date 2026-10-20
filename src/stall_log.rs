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
}

/// One app-driven pause; [Hold::end] logs how long playback stayed paused.
#[derive(Clone, Copy)]
pub(crate) struct Hold {
    reason: &'static str,
    since: Instant,
}

impl Hold {
    pub(crate) fn begin(reason: &'static str, mpv: &libmpv2::Mpv) -> Self {
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
