//! Baked-in black-strip detection: packed-frame crop (thumbs) and lavfi `cropdetect` (Fill Screen).

use libmpv2::Mpv;
use std::cell::Cell;
use std::ffi::{CStr, CString};
use std::rc::Rc;
use std::time::Duration;

use crate::mpv_embed::MpvBundle;

/// Reject crops that leave less than this fraction of width or height.
const MIN_CONTENT_FRAC: f64 = 0.5;
/// Ignore strips thinner than this fraction of the frame.
const MIN_BAR_FRAC: f64 = 0.02;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CropRect {
    pub w: i64,
    pub h: i64,
    pub x: i64,
    pub y: i64,
}

impl CropRect {
    fn as_video_crop(self) -> String {
        format!("{}x{}+{}+{}", self.w, self.h, self.x, self.y)
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
    /// True after the intro delay callback runs (ready-wait / gather may start).
    past_delay: Cell<bool>,
    /// True once cropdetect was inserted for this gen (blocks double insert).
    gathering: Cell<bool>,
}

impl BarProbe {
    pub fn new() -> Self {
        Self {
            state: Cell::new(BarState::Unknown),
            gen: Cell::new(0),
            ready_left: Cell::new(0),
            past_delay: Cell::new(false),
            gathering: Cell::new(false),
        }
    }

    pub fn invalidate(&self) {
        self.gen.set(self.gen.get().wrapping_add(1));
        self.state.set(BarState::Unknown);
        self.ready_left.set(0);
        self.past_delay.set(false);
        self.gathering.set(false);
    }

    fn start_gen(&self) -> u64 {
        let gen = self.gen.get().wrapping_add(1);
        self.gen.set(gen);
        self.state.set(BarState::Pending);
        self.ready_left.set(READY_RETRY_MAX);
        self.past_delay.set(false);
        self.gathering.set(false);
        gen
    }

    pub fn has_crop(&self) -> bool {
        matches!(self.state.get(), BarState::Crop(_))
    }

    pub fn crop(&self) -> Option<CropRect> {
        match self.state.get() {
            BarState::Crop(r) => Some(r),
            _ => None,
        }
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
