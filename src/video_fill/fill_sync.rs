//! [`FillSync`] state machine: button visibility and panscan application.

use super::{monitor_ar, FillSync, AR_TOLERANCE};
use gtk::prelude::*;

impl FillSync {
    /// Recheck visibility; apply or reset panscan to match user preference.
    pub(super) fn sync(&self) {
        let is_fs = self.win.is_fullscreen();
        let mismatch = self.aspect_mismatch();
        let show = is_fs && mismatch;
        if show {
            self.apply_panscan(self.preferred.get());
        } else if self.active.get() {
            self.reset_panscan();
        }
        self.btn.set_visible(show);
        if is_fs && !mismatch {
            if let Some(ar) = monitor_ar(&self.win) {
                eprintln!("[rhino] fill: fullscreen but no AR mismatch (monitor={ar:.3})");
            }
        }
    }

    /// Clear preference on new media so fill doesn't carry over across unrelated videos.
    pub(super) fn reset_preferred(&self) {
        self.preferred.set(false);
        if self.active.get() {
            self.reset_panscan();
        }
        self.btn.set_visible(false);
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

    pub(super) fn apply_panscan(&self, on: bool) {
        self.active.set(on);
        self.preferred.set(on);
        if let Some(b) = self.player.borrow().as_ref() {
            let v: f64 = if on { 1.0 } else { 0.0 };
            if let Err(e) = b.mpv.set_property("panscan", v) {
                eprintln!("[rhino] fill: panscan set failed: {e}");
            }
        }
        if on {
            self.btn.add_css_class("rp-fill-on");
        } else {
            self.btn.remove_css_class("rp-fill-on");
        }
    }

    fn reset_panscan(&self) {
        self.active.set(false);
        if let Some(b) = self.player.borrow().as_ref() {
            let _ = b.mpv.set_property("panscan", 0.0f64);
        }
        self.btn.remove_css_class("rp-fill-on");
    }
}
