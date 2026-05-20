// `MpvBundle` persistence + load methods. Split out of `main_bundle_egl_render.rs` so the
// platform-shaped construction code stays focused. `include!`'d at module level so it
// extends `MpvBundle` with another `impl` block (Rust forbids `include!` inside an impl).

impl MpvBundle {

/// Remember [Path] the shell just opened for ME budget + **`media`** row lookup (not read from mpv).
pub(crate) fn set_me_budget_shell_path(&self, path: &Path) {
    *self.me_budget_shell_path.borrow_mut() = std::fs::canonicalize(path).ok();
}

pub(crate) fn set_skip_media_persist(&self, skip: bool) {
    self.skip_media_persist.set(skip);
}

pub(crate) fn warm_file_gen(&self) -> u32 {
    self.warm_file_gen.get()
}

#[must_use]
pub(crate) fn may_persist_media_rows(&self) -> bool {
    !self.skip_media_persist.get()
}

/// End playback; call after the SQLite snapshot. Safe to skip before process exit.
pub fn stop_playback(&self) {
    *self.me_budget_shell_path.borrow_mut() = None;
    let _ = self.mpv.command("stop", &[]);
}

fn snapshot_playback_inner(&self) {
    if let Some(p) = media_probe::local_file_from_mpv(&self.mpv) {
        if media_probe::is_natural_end(&self.mpv) {
            media_probe::clear_resume_for_path(&p);
            return;
        }
    }
    media_probe::record_playback_for_current(&self.mpv);
}

/// Persist `duration` + `time-pos` unless [Self::skip_media_persist] (continue-grid warm hover).
pub fn save_playback_state(&self) {
    if self.skip_media_persist.get() {
        return;
    }
    self.snapshot_playback_inner();
}

/// Close / quit / back-from-playback: always persist the open file.
pub fn save_playback_state_for_close(&self) {
    self.snapshot_playback_inner();
}

/// Save SQLite resume snapshot, then stop playback. Used at process quit.
pub fn commit_quit(&self) {
    if !self.skip_media_persist.get() {
        self.save_playback_state_for_close();
    }
    self.stop_playback();
}

/// Save outgoing resume snapshot before leaving the open file (e.g. **Back to Browse**).
pub fn snapshot_outgoing_before_leave(&self) {
    self.save_playback_state_for_close();
}

/// Save outgoing resume to SQLite, then `loadfile` the new path. The new file's resume position
/// (if any in SQLite) is stashed in [pending_resume]; [apply_pending_resume] consumes it after
/// `FileLoaded`. We do **not** pass `start=` as a loadfile option — older mpv (≤ 0.35) treats
/// the third positional argument as `<index>` and rejects the whole command.
/// When [clear_outgoing_resume] is true, the outgoing file reached the end: drop its DB resume.
/// When [warm_preload] is true (continue-grid hover / first-card preload), do not snapshot or
/// clear the outgoing file — mpv is often still at 0s while paused behind the grid.
pub fn load_file_path(
    &self,
    path: &Path,
    clear_outgoing_resume: bool,
    snapshot_outgoing: bool,
    warm_preload: bool,
) -> Result<(), String> {
    if clear_outgoing_resume && !warm_preload {
        if let Some(p) = media_probe::local_file_from_mpv(&self.mpv) {
            media_probe::clear_resume_for_path(&p);
        }
    } else if snapshot_outgoing && !warm_preload {
        media_probe::record_playback_for_current(&self.mpv);
    }
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canonical.to_str().ok_or("media path is not valid UTF-8")?;
    if warm_preload {
        self.warm_file_gen.set(self.warm_file_gen.get().wrapping_add(1));
    }
    self.pending_resume.set(db::resume_pos(&canonical));
    self.mpv
        .command("loadfile", &[s, "replace"])
        .map_err(|e| format!("{e:?}"))
}

/// Apply the resume stashed by the most recent [load_file_path]. Idempotent and a no-op when
/// nothing is pending. Call once per `FileLoaded` from the transport-event drain.
/// Uses **`absolute+exact`** so the demuxer lands on the saved time (keyframe-only seeks can
/// sit at 0s briefly on load and fight the continue grid).
pub fn apply_pending_resume(&self) -> Option<f64> {
    let Some(path) = crate::media_probe::local_file_from_mpv(&self.mpv) else {
        return None;
    };
    if self.pending_resume.get().is_none() {
        let pos = self.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
        if pos.is_finite() && pos < crate::media_probe::NEAR_END_SEC {
            if let Some(t) = db::resume_pos(&path) {
                self.pending_resume.set(Some(t));
            }
        }
    }
    if self.pending_resume.get().is_none() {
        return None;
    }
    let Some(t) = self.pending_resume.replace(None) else {
        return None;
    };
    let _ = crate::video_pref::unload_smooth_on_pause(&self.mpv);
    let s = format!("{t:.4}");
    let _ = self.mpv.command("seek", &[s.as_str(), "absolute+exact"]);
    Some(t)
}

/// Seek knob / elapsed while paused on the continue grid — SQLite resume only (mpv `time-pos` lags).
pub(crate) fn knob_pos_from_sqlite(&self) -> f64 {
    let path = crate::media_probe::local_file_from_mpv(&self.mpv)
        .or_else(|| self.me_budget_shell_path.borrow().clone());
    path.and_then(|p| db::resume_pos(&p)).unwrap_or(0.0)
}

}
