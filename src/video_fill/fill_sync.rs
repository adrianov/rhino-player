//! [`FillSync`] state machine: button visibility, panscan, and baked-in bar crop.

use crate::black_bars::{
    apply_video_crop, clear_video_crop, pump_bar_probe, schedule_bar_probe, BarState,
};
use super::{monitor_ar, stored_fill_preference, FillSync, AR_TOLERANCE};
use gtk::prelude::*;
use std::rc::Rc;

impl FillSync {
    /// Recheck visibility; apply or reset fill to match user preference.
    pub(super) fn sync(&self) {
        let is_fs = self.win.is_fullscreen();
        let can_fill = self.can_fill();
        let show = is_fs && can_fill;
        if show {
            self.apply_fill(self.preferred.get());
        } else if self.active.get() {
            self.reset_fill_view();
        }
        self.btn.set_visible(show);
    }

    /// New media opened: clear crop + view, re-arm preferred from DB, start strip probe.
    pub(super) fn reset_preferred(&self) {
        self.preferred.set(stored_fill_preference(&self.player));
        self.bars.invalidate();
        if let Some(b) = self.player.borrow().as_ref() {
            clear_video_crop(&b.mpv);
        }
        if self.active.get() {
            self.reset_fill_view();
        }
        self.btn.set_visible(false);
        self.kick_bar_probe();
    }

    /// FileLoaded / reconfig: start or resume strip probe, then sync visibility.
    pub(super) fn on_media_ready(&self) {
        match self.bars.state.get() {
            BarState::Unknown => self.kick_bar_probe(),
            BarState::Pending => self.resume_bar_probe(),
            BarState::Clean | BarState::Crop(_) => {}
        }
        self.sync();
    }

    fn kick_bar_probe(&self) {
        schedule_bar_probe(
            &self.player,
            &self.bars,
            Rc::new(super::request_fill_sync_only),
        );
    }

    fn resume_bar_probe(&self) {
        pump_bar_probe(
            &self.player,
            &self.bars,
            Rc::new(super::request_fill_sync_only),
        );
    }

    fn can_fill(&self) -> bool {
        self.aspect_mismatch() || self.bars.has_crop()
    }

    fn aspect_mismatch(&self) -> bool {
        let guard = self.player.borrow();
        let Some(b) = guard.as_ref() else {
            return false;
        };
        let Some(screen_ar) = monitor_ar(&self.win) else {
            return false;
        };
        let Ok(vw) = b.mpv.get_property::<i64>("dwidth") else {
            return false;
        };
        let Ok(vh) = b.mpv.get_property::<i64>("dheight") else {
            return false;
        };
        if vw <= 0 || vh <= 0 {
            return false;
        }
        (screen_ar - vw as f64 / vh as f64).abs() > AR_TOLERANCE
    }

    pub(super) fn apply_fill(&self, on: bool) {
        self.active.set(on);
        self.preferred.set(on);
        if let Some(b) = self.player.borrow().as_ref() {
            if on {
                apply_video_crop(&b.mpv, self.bars.crop());
                if let Err(e) = b.mpv.set_property("panscan", 1.0f64) {
                    eprintln!("[rhino] fill: panscan set failed: {e}");
                }
            } else {
                clear_video_crop(&b.mpv);
                if let Err(e) = b.mpv.set_property("panscan", 0.0f64) {
                    eprintln!("[rhino] fill: panscan set failed: {e}");
                }
            }
        }
        if on {
            self.btn.add_css_class("rp-fill-on");
        } else {
            self.btn.remove_css_class("rp-fill-on");
        }
    }

    fn reset_fill_view(&self) {
        self.active.set(false);
        if let Some(b) = self.player.borrow().as_ref() {
            clear_video_crop(&b.mpv);
            let _ = b.mpv.set_property("panscan", 0.0f64);
        }
        self.btn.remove_css_class("rp-fill-on");
    }
}
