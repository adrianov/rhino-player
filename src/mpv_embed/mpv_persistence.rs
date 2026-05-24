// `MpvBundle` persistence + load methods. Split out of `main_bundle_egl_render.rs` so the
// platform-shaped construction code stays focused. `include!`'d at module level so it
// extends `MpvBundle` with another `impl` block (Rust forbids `include!` inside an impl).

impl MpvBundle {

fn persist_media_path(&self) -> Option<std::path::PathBuf> {
    media_probe::shell_media_path(
        &self.mpv,
        self.me_budget_shell_path.borrow().as_deref(),
    )
}

/// Remember [Path] the shell just opened for ME budget + **`media`** row lookup (not read from mpv).
pub(crate) fn set_me_budget_shell_path(&self, path: &Path) {
    *self.me_budget_shell_path.borrow_mut() = Some(
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
    );
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
    media_probe::record_playback_for_current(&self.mpv, self.me_budget_shell_path.borrow().as_deref());
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
    resume_at: Option<f64>,
) -> Result<(), String> {
    {
        let shell = self.me_budget_shell_path.borrow();
        let outgoing = media_probe::shell_media_path(&self.mpv, shell.as_deref());
        if clear_outgoing_resume && !warm_preload {
            if let Some(p) = outgoing {
                media_probe::clear_resume_for_path(&p);
            }
        } else if snapshot_outgoing && !warm_preload {
            media_probe::record_playback_for_current(&self.mpv, shell.as_deref());
        }
    }
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let entity = crate::playback_entity::PlaybackEntity::resolve(&canonical);
    let db_key = entity.db_path();
    let stored = resume_at.or_else(|| db::resume_pos(&db_key));
    let (load_path, pending) = if let Some(global) = stored {
        let map = db::load_duration_map();
        entity
            .resume_load_target(&canonical, global, &map)
            .map(|(target, local)| (target, Some(local)))
            .unwrap_or((canonical.clone(), None))
    } else {
        (canonical.clone(), None)
    };
    let s = load_path.to_str().ok_or("media path is not valid UTF-8")?;
    if warm_preload {
        self.warm_file_gen.set(self.warm_file_gen.get().wrapping_add(1));
    }
    self.clear_chapter_scrub_pause_hold();
    self.chapter_scrub_resume.set(false);
    self.pending_resume.set(pending);
    self.set_me_budget_shell_path(&load_path);
    self.mpv
        .command("loadfile", &[s, "replace"])
        .map_err(|e| format!("{e:?}"))
}

/// Cross-chapter DVD seek: `loadfile` with chapter-local resume (not entity-global remap).
pub fn load_chapter_seek(
    &self,
    path: &Path,
    local_sec: f64,
    hold_global: f64,
    resume_playing: bool,
    chapter_eof: bool,
) -> Result<(), String> {
    {
        let shell = self.me_budget_shell_path.borrow();
        let outgoing = media_probe::shell_media_path(&self.mpv, shell.as_deref());
        if outgoing.is_some() {
            media_probe::record_playback_for_current(&self.mpv, shell.as_deref());
        }
    }
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canonical.to_str().ok_or("media path is not valid UTF-8")?;
    self.dvd_hold_global.set(Some(hold_global));
    self.chapter_eof_load.set(chapter_eof);
    self.chapter_scrub_resume.set(true);
    self.begin_chapter_scrub_pause_hold(resume_playing);
    self.pending_resume.set(Some(local_sec.max(0.0)));
    self.set_me_budget_shell_path(&canonical);
    crate::video_pref::strip_vapoursynth_before_replace_media(self);
    crate::dvd_vob_log::dvd_seek_log(format!(
        "load_chapter_seek file={} local={local_sec:.2} hold_global={hold_global:.2}",
        canonical.display()
    ));
    if let Err(e) = self.mpv.command("loadfile", &[s, "replace"]) {
        self.abort_chapter_load(true);
        return Err(format!("{e:?}"));
    }
    Ok(())
}

/// Chapter-local resume seconds from SQLite for the open shell path (warm reopen fallback).
fn stored_resume_local_for_shell(&self) -> Option<f64> {
    let shell = self.me_budget_shell_path.borrow().clone()?;
    let canonical = std::fs::canonicalize(&shell).unwrap_or(shell);
    let entity = crate::playback_entity::PlaybackEntity::resolve(&canonical);
    let stored = db::resume_pos(&entity.db_path())?;
    let map = db::load_duration_map();
    let (target, local) = entity.resume_load_target(&canonical, stored, &map)?;
    let open = media_probe::shell_media_path(
        &self.mpv,
        self.me_budget_shell_path.borrow().as_deref(),
    )?;
    if !crate::video_ext::paths_same_file(&target, &open) {
        return None;
    }
    Some(local)
}

fn clear_pending_resume_done(&self) {
    self.pending_resume.set(None);
    self.dvd_hold_global.set(None);
}

fn resume_wait_duration(&self, chapter_scrub: bool, pending_t: f64) -> f64 {
    let mut dur = self
        .mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    if dur <= 0.0 && chapter_scrub {
        dur = self.chapter_scrub_demux_duration();
    }
    if dur <= 0.0 && chapter_scrub {
        if let Some(shell) = self.me_budget_shell_path.borrow().clone() {
            dur = crate::dvd_vob_timeline::dur_from_map(
                &crate::db::load_duration_map(),
                shell.as_path(),
            );
        }
    }
    if dur <= 0.0 && chapter_scrub && pending_t > 0.0 {
        dur = pending_t + 1.0;
    }
    dur
}

fn apply_file_pending_resume(&self) -> Option<f64> {
    if let Some(ref p) = self.persist_media_path() {
        resume_seek::stash_near_start_resume(&self.mpv, &self.pending_resume, p);
    }
    let t = self.pending_resume.get()?;
    let pos = self.mpv.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
    if resume_seek::resume_already_at(&self.mpv, t) {
        self.clear_pending_resume_done();
        crate::dvd_vob_log::dvd_seek_log(format!(
            "apply_pending_resume: at target {t:.2} (pos={pos:.2})"
        ));
        return Some(t);
    }
    resume_seek::seek_to_resume_sec(&self.mpv, t);
    if resume_seek::resume_already_at(&self.mpv, t) {
        self.clear_pending_resume_done();
    }
    crate::dvd_vob_log::dvd_seek_log(format!("apply_pending_resume: seek {t:.2} (was pos={pos:.2})"));
    Some(t)
}

/// Open mpv `path` matches [Self::set_me_budget_shell_path] (set before `loadfile`).
fn mpv_path_matches_shell(&self) -> bool {
    let shell = self.me_budget_shell_path.borrow();
    let Some(ref target) = *shell else {
        return true;
    };
    media_probe::mpv_matches_open_target(&self.mpv, shell.as_deref(), target.as_path())
}

/// Apply the resume stashed by the most recent [load_file_path] or [load_chapter_seek]. Idempotent
/// and a no-op when nothing is pending. Call from `FileLoaded` and again on `path` / `duration`
/// when the shell path was set before mpv switched files (cross-chapter DVD scrub).
/// Uses **`absolute+exact`** so the demuxer lands on the saved time (keyframe-only seeks can
/// sit at 0s briefly on load and fight the continue grid).
pub fn apply_pending_resume(&self) -> Option<f64> {
    let Some(t) = self.pending_resume.get() else {
        self.dvd_hold_global.set(None);
        self.chapter_scrub_resume.set(false);
        if self.chapter_scrub_hold_pause.get() {
            self.finish_chapter_scrub_pause_hold();
        }
        return None;
    };
    if !self.mpv_path_matches_shell() {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "apply_pending_resume: wait path (target={t:.2})"
        ));
        return None;
    }
    let chapter_scrub = self.chapter_scrub_resume.get();
    let pending_t = self.pending_resume.get().unwrap_or(0.0);
    let dur = self.resume_wait_duration(chapter_scrub, pending_t);
    if dur <= 0.0 {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "apply_pending_resume: wait duration (target={pending_t:.2})"
        ));
        return None;
    }
    let t = self.pending_resume.get()?;
    if chapter_scrub {
        return self.apply_chapter_scrub_pending_resume(t);
    }
    self.apply_file_pending_resume()
}

/// True when the pending `loadfile` is a same-title DVD chapter advance (EOF auto-load).
#[must_use]
pub fn take_chapter_eof_load(&self) -> bool {
    self.chapter_eof_load.replace(false)
}

/// Warm reopen (card click / Space): SQLite fallback when preload cleared pending before seek landed.
pub fn apply_pending_resume_on_warm_open(&self) -> Option<f64> {
    if !self.mpv_path_matches_shell() || self.pending_resume.get().is_some() {
        return None;
    }
    if !media_probe::mpv_has_known_duration(&self.mpv) {
        return None;
    }
    let t = self.stored_resume_local_for_shell()?;
    if resume_seek::resume_already_at(&self.mpv, t) {
        return Some(t);
    }
    resume_seek::seek_to_resume_sec(&self.mpv, t);
    Some(t)
}

/// Continue-grid reveal / card open: apply stashed or SQLite resume before unpausing.
pub fn ensure_resume_before_unpause(&self) -> Option<f64> {
    if let Some(t) = self.apply_pending_resume() {
        return Some(t);
    }
    if self.pending_resume.get().is_some() {
        return None;
    }
    self.apply_pending_resume_on_warm_open()
}

}
