//! [`FillSync`] state machine: button visibility, panscan, and baked-in bar crop.

use super::{current_local_media_path, stored_fill_preference, viewport_ar, FillSync, AR_TOLERANCE};
use crate::black_bars::{apply_video_crop, clear_video_crop, BarState};
use gtk::prelude::*;
use std::rc::Rc;

mod probe;

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

    /// Recheck visibility; keep strip crop applied; match panscan to preference.
    pub(super) fn sync(&self) {
        let show = self.visibility_show();
        self.log_show_change(show);
        // Unknown content AR (decode size missing during Bob/reconfig) — do not clear panscan.
        let Some(show) = show else {
            return;
        };
        // Window fit/snap uses strip-free content AR; crop must follow even when Fill is off,
        // otherwise a cinematic window pillarboxes the full coded frame and Fill stays hidden.
        self.sync_bar_crop();
        if show {
            let want = self.preferred.get();
            // Re-apply when on so a late strip crop attaches; skip no-op fitted syncs.
            if want || self.active.get() {
                self.apply_panscan(want);
            }
        } else if self.active.get() {
            self.apply_panscan(false);
        }
        self.btn.set_visible(show);
    }

    /// `Some(true)` when the viewport aspect diverges from content aspect beyond tolerance.
    fn visibility_show(&self) -> Option<bool> {
        let view_ar = self.viewport.borrow().as_ref().and_then(viewport_ar);
        match (view_ar, self.content_ar()) {
            (Some(v), Some(c)) => Some((v - c).abs() > AR_TOLERANCE),
            _ => None,
        }
    }

    /// Log only when the visibility verdict flips — resize/reconfig bursts stay quiet.
    fn log_show_change(&self, show: Option<bool>) {
        if self.last_show.replace(show) != show {
            let view_ar = self.viewport.borrow().as_ref().and_then(viewport_ar);
            eprintln!(
                "[rhino] fill: viewport {view_ar:?} vs content {:?} -> show={show:?} pref={} act={}",
                self.content_ar(),
                self.preferred.get(),
                self.active.get()
            );
        }
    }

    /// New media opened: clear crop + view, re-arm preferred from DB, start strip probe.
    /// A sibling transition (target bound by `video_fill::request_fill_carry`) keeps the
    /// current intent when the new video has no stored choice of its own; the carry
    /// applies only when the opened media is the bound target.
    pub(super) fn reset_preferred(&self) {
        let carry_target = super::carry::take_fill_carry_target();
        let stored = stored_fill_preference(&self.player);
        let carry = super::carry::carry_applies(
            carry_target,
            current_local_media_path(&self.player),
            self.preferred.get(),
        );
        let next = stored.unwrap_or(carry);
        eprintln!(
            "[rhino] fill: reset pref {} -> {next} (stored={stored:?} carry={carry})",
            self.preferred.get()
        );
        self.preferred.set(next);
        self.bars.invalidate();
        *self.last_crop_spec.borrow_mut() = None;
        if let Some(b) = self.player.borrow().as_ref() {
            clear_video_crop(&b.mpv);
        }
        if self.active.get() {
            self.apply_panscan(false);
        }
        self.btn.set_visible(false);
        self.kick_bar_probe();
    }

    /// FileLoaded / reconfig: start or resume strip probe, then sync visibility.
    /// The unpause marker is consumed on every entry so a later unrelated
    /// reconfiguration cannot inherit it and bypass the probe's own-cleanup
    /// suppression; only the Pending branch acts on it (see `pump_bar_probe`).
    pub(super) fn on_media_ready(&self) {
        match self.bars.state.get() {
            BarState::Unknown => {
                super::take_resync_after_unpause();
                self.kick_bar_probe();
            }
            BarState::Pending => self.resume_bar_probe(super::take_resync_after_unpause()),
            BarState::Clean | BarState::Crop(_) => {
                super::take_resync_after_unpause();
                if let Some(b) = self.player.borrow().as_ref() {
                    if self.bars.needs_deint_reprobe(&b.mpv) {
                        eprintln!("[rhino] bars: re-probe after Bob deinterlace attached");
                        self.kick_bar_probe_live();
                    }
                }
            }
        }
        self.sync();
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

    /// Keep `video-crop` aligned with the strip probe: apply on `Crop`, clear on `Clean`.
    /// Leave `Unknown` / `Pending` alone so a mid-probe or just-reset media is not stomped.
    /// Re-apply on every sync while `Crop` so VO remapping tracks Smooth/reconfig; log change-only.
    fn sync_bar_crop(&self) {
        let player = self.player.borrow();
        let Some(b) = player.as_ref() else {
            return;
        };
        match self.bars.state.get() {
            BarState::Crop(rect) => {
                // Re-apply every sync so VO remapping tracks Smooth/reconfig; log once per spec.
                let spec = rect.as_video_crop();
                let first = self.last_crop_spec.borrow().as_deref() != Some(spec.as_str());
                apply_video_crop(&b.mpv, Some(rect));
                if first {
                    eprintln!("[rhino] bars: video-crop applied {spec} (fitted or fill)");
                    *self.last_crop_spec.borrow_mut() = Some(spec);
                }
            }
            BarState::Clean => {
                if self.last_crop_spec.borrow().is_none() {
                    return;
                }
                clear_video_crop(&b.mpv);
                eprintln!("[rhino] bars: video-crop cleared (probe clean)");
                *self.last_crop_spec.borrow_mut() = None;
            }
            BarState::Unknown | BarState::Pending => {}
        }
    }

    pub(super) fn apply_fill(&self, on: bool) {
        self.preferred.set(on);
        self.sync_bar_crop();
        self.apply_panscan(on);
    }

    fn apply_panscan(&self, on: bool) {
        self.active.set(on);
        if let Some(b) = self.player.borrow().as_ref() {
            let pan = if on { 1.0f64 } else { 0.0f64 };
            if let Err(e) = b.mpv.set_property("panscan", pan) {
                eprintln!("[rhino] fill: panscan set failed: {e}");
            }
        }
        if on {
            self.btn.add_css_class("rp-fill-on");
        } else {
            self.btn.remove_css_class("rp-fill-on");
        }
    }
}
