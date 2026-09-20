//! Baked-in black-strip detection: packed-frame crop (thumbs) and lavfi `cropdetect` (Fill Screen).

use libmpv2::Mpv;
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::mpv_embed::MpvBundle;

/// Reject crops that leave less than this fraction of width or height.
const MIN_CONTENT_FRAC: f64 = 0.5;
/// Ignore strips thinner than this fraction of the frame.
const MIN_BAR_FRAC: f64 = 0.02;
/// How long a probe's own teardown may swallow matching reconfigs. Scoped so a
/// later decoder readiness change or filter rebuild that keeps the same vf
/// chain still re-arms the probe instead of being mistaken for cleanup.
const RECONFIG_SETTLE_WINDOW: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CropRect {
    pub w: i64,
    pub h: i64,
    pub x: i64,
    pub y: i64,
}

impl CropRect {
    pub fn as_video_crop(self) -> String {
        format!("{}x{}+{}+{}", self.w, self.h, self.x, self.y)
    }

    /// Parse mpv `video-crop` / lavfi style `WxH+X+Y`.
    pub fn parse_video_crop(spec: &str) -> Option<Self> {
        let [w, h, x, y] = crop_spec_parts(spec)?;
        Some(Self { w, h, x, y })
    }
}

fn crop_spec_parts(spec: &str) -> Option<[i64; 4]> {
    let mut out = [0i64; 4];
    let mut n = 0usize;
    for part in spec.split(['x', '+']) {
        if n >= 4 {
            return None;
        }
        out[n] = part.parse().ok()?;
        n += 1;
    }
    (n == 4).then_some(out)
}

#[cfg(test)]
mod crop_spec_tests {
    use super::CropRect;

    #[test]
    fn crop_rect_parses_mpv_video_crop() {
        assert_eq!(
            CropRect::parse_video_crop("1920x800+0+140"),
            Some(CropRect {
                w: 1920,
                h: 800,
                x: 0,
                y: 140
            })
        );
        assert_eq!(CropRect::parse_video_crop(""), None);
        assert_eq!(CropRect::parse_video_crop("1920x800"), None);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarState {
    /// Probe not started (or cancelled).
    Unknown,
    /// Delay / ready-wait / cropdetect gather in flight.
    Pending,
    /// Probe finished; no meaningful strips.
    Clean,
    /// Meaningful baked-in strips.
    Crop(CropRect),
}

/// Shared probe result for the current media (generation bumps cancel in-flight work).
pub struct BarProbe {
    pub state: Cell<BarState>,
    gen: Cell<u64>,
    /// Remaining waits for decode size after the initial detect delay.
    ready_left: Cell<u8>,
    /// Remaining retries when a gather ends without cropdetect metadata (paused / rebuild).
    metadata_retries: Cell<u8>,
    /// True after the intro delay callback runs (ready-wait / gather may start).
    past_delay: Cell<bool>,
    /// True once cropdetect was inserted for this gen (blocks double insert).
    gathering: Cell<bool>,
    /// Finished probe saw Bob (`rhino-deint`) in the vf chain (else re-arm after Bob attaches).
    saw_deint: Cell<bool>,
    /// vf chain right after this probe's own teardown (cropdetect removal / hwdec
    /// restore), with the settle timestamp: matching reconfigs are the probe's own
    /// cleanup events only inside the settle window — never an open-ended veto.
    settled_vf: RefCell<Option<(String, Instant)>>,
}

impl BarProbe {
    pub fn new() -> Self {
        Self {
            state: Cell::new(BarState::Unknown),
            gen: Cell::new(0),
            ready_left: Cell::new(0),
            metadata_retries: Cell::new(0),
            past_delay: Cell::new(false),
            gathering: Cell::new(false),
            saw_deint: Cell::new(false),
            settled_vf: RefCell::new(None),
        }
    }

    pub fn invalidate(&self) {
        self.gen.set(self.gen.get().wrapping_add(1));
        self.state.set(BarState::Unknown);
        self.ready_left.set(0);
        self.metadata_retries.set(0);
        self.past_delay.set(false);
        self.gathering.set(false);
        self.saw_deint.set(false);
        self.settled_vf.take();
    }

    /// Apply a DB-cached Clean/Crop result and cancel any in-flight probe.
    pub fn restore_cached(&self, state: BarState, saw_deint: bool) {
        debug_assert!(matches!(state, BarState::Clean | BarState::Crop(_)));
        self.gen.set(self.gen.get().wrapping_add(1));
        self.state.set(state);
        self.ready_left.set(0);
        self.metadata_retries.set(0);
        self.past_delay.set(false);
        self.gathering.set(false);
        // Preserve probe-time Bob flag so a pre-deint cache can still re-arm.
        self.saw_deint.set(saw_deint);
        self.settled_vf.take();
    }

    pub fn saw_deint(&self) -> bool {
        self.saw_deint.get()
    }

    fn start_gen(&self) -> u64 {
        let gen = self.gen.get().wrapping_add(1);
        self.gen.set(gen);
        self.state.set(BarState::Pending);
        self.ready_left.set(READY_RETRY_MAX);
        self.metadata_retries.set(BAR_META_RETRIES);
        self.past_delay.set(false);
        self.gathering.set(false);
        self.saw_deint.set(false);
        self.settled_vf.take();
        gen
    }

    pub fn crop(&self) -> Option<CropRect> {
        match self.state.get() {
            BarState::Crop(r) => Some(r),
            _ => None,
        }
    }

    /// True when a finished probe should run again because Bob attached after it.
    pub fn needs_deint_reprobe(&self, mpv: &Mpv) -> bool {
        if self.saw_deint.get() {
            return false;
        }
        if !matches!(self.state.get(), BarState::Clean | BarState::Crop(_)) {
            return false;
        }
        crate::video_pref::bob_deinterlace_in_vf(
            &mpv.get_property::<String>("vf").unwrap_or_default(),
        )
    }
    /// Consume one no-frame-data retry; `false` when the budget is exhausted.
    pub fn take_meta_retry(&self) -> bool {
        let left = self.metadata_retries.get();
        if left == 0 {
            return false;
        }
        self.metadata_retries.set(left - 1);
        true
    }

    /// Record the vf chain left behind by this probe's teardown; matching reconfigs
    /// count as the probe's own cleanup events for a short settle window only.
    pub fn settle_cleanup_vf(&self, vf: String) {
        self.settle_cleanup_vf_at(vf, Instant::now());
    }

    /// `settle_cleanup_vf` with an injectable clock (tests backdate the record).
    pub(crate) fn settle_cleanup_vf_at(&self, vf: String, at: Instant) {
        *self.settled_vf.borrow_mut() = Some((vf, at));
    }

    /// `true` only while a `VideoReconfig` carrying this vf chain still falls
    /// inside the settle window opened by the probe's own teardown. After the
    /// window, an unchanged-chain reconfig is external (decoder readiness change,
    /// filter rebuild) and must re-arm the probe.
    pub fn reconfig_is_cleanup(&self, vf: Option<&str>) -> bool {
        let settled = self.settled_vf.borrow();
        let Some((chain, at)) = settled.as_ref() else {
            return false;
        };
        at.elapsed() < RECONFIG_SETTLE_WINDOW && matches!(vf, Some(vf) if vf == chain)
    }
}

#[cfg(test)]
mod probe_gate_tests {
    use super::BarProbe;

    #[test]
    fn cleanup_reconfig_suppression_follows_the_probe_cycle() {
        let p = BarProbe::new();
        // No settled chain yet — any reconfig is external.
        assert!(!p.reconfig_is_cleanup(Some("smooth")));
        // After a gather's teardown, its own cleanup events are suppressed
        // (mpv may emit several) while a changed chain re-arms.
        p.settle_cleanup_vf("a".into());
        assert!(p.reconfig_is_cleanup(Some("a")));
        assert!(p.reconfig_is_cleanup(Some("a")));
        assert!(!p.reconfig_is_cleanup(Some("a:bob")));
        assert!(!p.reconfig_is_cleanup(None));
        // A new probe cycle forgets the previous teardown signature.
        p.start_gen();
        assert!(!p.reconfig_is_cleanup(Some("a")));
        p.settle_cleanup_vf("b".into());
        p.invalidate();
        assert!(!p.reconfig_is_cleanup(Some("b")));
    }

    #[test]
    fn cleanup_suppression_expires_so_unchanged_chains_re_arm() {
        let p = BarProbe::new();
        let now = std::time::Instant::now();
        // Fresh teardown: matching chain swallowed inside the window...
        p.settle_cleanup_vf_at("a".into(), now);
        assert!(p.reconfig_is_cleanup(Some("a")));
        // ...but afterwards a decoder readiness change or filter rebuild that
        p.settle_cleanup_vf_at(
            "a".into(),
            now.checked_sub(super::RECONFIG_SETTLE_WINDOW + std::time::Duration::from_millis(1))
                .expect("monotonic clock covers the window"),
        );
        assert!(!p.reconfig_is_cleanup(Some("a")));
    }

    #[test]
    fn scaled_crop_follows_the_chain_output_image() {
        // Decode 1920x1080, cached 1784x890+0+56, Smooth60 vo image 1552x872.
        let rect = super::CropRect { w: 1784, h: 890, x: 0, y: 56 };
        let out = super::scale_rect_between(rect, (1920, 1080), (1552, 872));
        assert_eq!((out.w, out.h, out.x, out.y), (1442, 719, 0, 45));
        // Mapping back keeps the strip fractions within a pixel of the source.
        let back = super::scale_rect_between(out, (1552, 872), (1920, 1080));
        assert!((back.w - rect.w).abs() <= 1 && (back.h - rect.h).abs() <= 1);
        assert!((back.x - rect.x).abs() <= 1 && (back.y - rect.y).abs() <= 1);
    }

    #[test]
    fn scaled_crop_clamps_into_the_target_image() {
        let rect = super::CropRect { w: 1920, h: 1080, x: 10, y: 20 };
        let out = super::scale_rect_between(rect, (1920, 1080), (1552, 872));
        assert_eq!((out.w, out.h, out.x, out.y), (1552, 872, 0, 0));
    }

    #[test]
    fn scaled_crop_is_identity_for_equal_spaces() {
        let rect = super::CropRect { w: 100, h: 50, x: 2, y: 3 };
        let out = super::scale_rect_between(rect, (200, 100), (200, 100));
        assert_eq!((out.w, out.h, out.x, out.y), (100, 50, 2, 3));
    }
}

fn crop_meaningful(fw: i64, fh: i64, cw: i64, ch: i64) -> bool {
    if cw <= 0 || ch <= 0 || (cw == fw && ch == fh) {
        return false;
    }
    let dw = (fw - cw) as f64 / fw as f64;
    let dh = (fh - ch) as f64 / fh as f64;
    if dw < MIN_BAR_FRAC && dh < MIN_BAR_FRAC {
        return false;
    }
    (cw as f64) >= fw as f64 * MIN_CONTENT_FRAC && (ch as f64) >= fh as f64 * MIN_CONTENT_FRAC
}

include!("black_bars/frame.rs");
include!("black_bars/probe.rs");
include!("black_bars/probe_defer.rs");
include!("black_bars/probe_finish.rs");
include!("black_bars/crop_space.rs");
