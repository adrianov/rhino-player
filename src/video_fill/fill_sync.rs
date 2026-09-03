//! [`FillSync`] state machine: button visibility, panscan, and baked-in bar crop.

use super::{stored_fill_preference, viewport_ar, FillSync, AR_TOLERANCE};
use crate::black_bars::{
    apply_video_crop, clear_video_crop, pump_bar_probe, schedule_bar_probe, BarState,
};
use gtk::prelude::*;
use std::rc::Rc;

impl FillSync {
    /// Wire the video surface for aspect checks and resize resync (once).
    pub(super) fn attach_viewport(self: &Rc<Self>, viewport: &gtk::GLArea) {
        *self.viewport.borrow_mut() = Some(viewport.clone());
        if !self.resize_hooked.replace(true) {
            let s = Rc::clone(self);
            viewport.connect_resize(move |_, _, _| {
                let s = Rc::clone(&s);
                let _ = glib::idle_add_local_once(move || s.sync());
            });
        }
        self.sync();
    }

    /// Recheck visibility; apply or reset fill to match user preference.
    pub(super) fn sync(&self) {
        // Unknown content AR (decode size missing during Bob/reconfig) — do not clear panscan.
        let Some(show) = self.aspect_mismatch() else {
            return;
        };
        if show {
            let want = self.preferred.get();
            // Re-apply when on so a late strip crop attaches; skip no-op fitted syncs.
            if want || self.active.get() {
                self.apply_fill(want);
            }
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
            BarState::Clean | BarState::Crop(_) => {
                if let Some(b) = self.player.borrow().as_ref() {
                    if self.bars.needs_deint_reprobe(&b.mpv) {
                        eprintln!("[rhino] bars: re-probe after Bob deinterlace attached");
                        self.kick_bar_probe();
                    }
                }
            }
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

    /// Viewport vs content aspect (strip crop when known, else decode size).
    /// `None` = sizes not ready yet (keep current fill; do not treat as "matched").
    fn aspect_mismatch(&self) -> Option<bool> {
        let view_ar = viewport_ar(self.viewport.borrow().as_ref()?)?;
        let content_ar = self.content_ar()?;
        Some((view_ar - content_ar).abs() > AR_TOLERANCE)
    }

    fn content_ar(&self) -> Option<f64> {
        if let Some(c) = self.bars.crop() {
            return (c.w > 0 && c.h > 0).then(|| c.w as f64 / c.h as f64);
        }
        self.player.borrow().as_ref().and_then(|b| {
            let vw = b.mpv.get_property::<i64>("dwidth").ok()?;
            let vh = b.mpv.get_property::<i64>("dheight").ok()?;
            (vw > 0 && vh > 0).then(|| vw as f64 / vh as f64)
        })
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
