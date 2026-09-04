// Pending resume lookups + state clearing after chapter scrub (`impl MpvBundle` extension).
// Apply/seek execution lives in `mpv_pending_resume_apply.rs`; shared shell-path and duration
// probes in `mpv_bundle_probes.rs`.

impl MpvBundle {
    pub(crate) fn smooth_vf_attach_pending(&self) -> bool {
        self.smooth_vf_attach_pending.get()
    }

    pub(crate) fn set_smooth_vf_attach_pending(&self, pending: bool) {
        self.smooth_vf_attach_pending.set(pending);
    }

    pub(crate) fn smooth_vf_stripped_this_open(&self) -> bool {
        self.smooth_vf_stripped_this_open.get()
    }

    pub(crate) fn set_smooth_vf_stripped_this_open(&self, stripped: bool) {
        self.smooth_vf_stripped_this_open.set(stripped);
    }

    pub(crate) fn clear_smooth_vf_stripped_this_open(&self) {
        self.smooth_vf_stripped_this_open.set(false);
    }

    pub(crate) fn smooth_vf_reload_attempted(&self) -> bool {
        self.smooth_vf_reload_attempted.get()
    }

    pub(crate) fn set_smooth_vf_reload_attempted(&self, attempted: bool) {
        self.smooth_vf_reload_attempted.set(attempted);
    }

    pub(crate) fn clear_smooth_vf_reload_attempted(&self) {
        self.smooth_vf_reload_attempted.set(false);
    }

    /// Chapter-local resume seconds from SQLite for the open shell path (warm reopen fallback).
    fn stored_resume_local_for_shell(&self) -> Option<f64> {
        let shell = self.me_budget_shell_path.borrow().clone()?;
        let (target, local) =
            resume_seek::stored_resume_target(&canonicalize_media_path(&shell))?;
        let open = media_probe::shell_media_path(
            &self.mpv,
            self.me_budget_shell_path.borrow().as_deref(),
        )?;
        if !crate::video_ext::paths_same_file(&target, &open) {
            return None;
        }
        Some(local)
    }

    fn stored_entity_global(&self) -> Option<f64> {
        let shell = self.me_budget_shell_path.borrow().clone()?;
        crate::db::resume_pos(
            &crate::playback_entity::PlaybackEntity::resolve(&shell).db_path(),
        )
    }

    /// Resume consumed: drop it and hand the final position to the chain-bar sync.
    fn clear_pending_resume_done(&self) {
        let ifo_local = self.pending_resume.get().unwrap_or(0.0);
        self.pending_resume.set(None);
        if !self.open_shell_is_chain_head() {
            self.dvd_hold_global.set(None);
            self.dvd_chain_bar_sync.set(None);
            return;
        }
        self.sync_chain_head_bar_after_clear(ifo_local);
    }

    /// Chain-head clear: seed the chain-bar sync from the held (or stored) global, then release it.
    fn sync_chain_head_bar_after_clear(&self, ifo_local: f64) {
        if let Some(global) = self
            .dvd_hold_global
            .get()
            .or_else(|| self.stored_entity_global())
        {
            self.dvd_chain_bar_sync.set(Some(
                crate::dvd_vob_timeline::DvdChainBarSync::from_targets(
                    ifo_local,
                    global,
                    self.current_playback_seconds(),
                ),
            ));
            self.persist_chain_head_total(global);
        }
        self.dvd_hold_global.set(None);
    }

    /// Live playback seconds, preferring `playback-time` and falling back to `time-pos` (0 if neither).
    fn current_playback_seconds(&self) -> f64 {
        self.mpv
            .get_property::<f64>("playback-time")
            .ok()
            .filter(|t| t.is_finite() && *t >= 0.0)
            .or_else(|| {
                self.mpv
                    .get_property::<f64>("time-pos")
                    .ok()
                    .filter(|t| t.is_finite() && *t >= 0.0)
            })
            .unwrap_or(0.0)
    }

    /// Persist the title-wide bar global for the open chain-head entity when its total is known.
    fn persist_chain_head_total(&self, global: f64) {
        let Some(shell) = self.me_budget_shell_path.borrow().clone() else {
            return;
        };
        let Some(total) = entity_row_duration(
            &crate::playback_entity::PlaybackEntity::resolve(&shell).db_path(),
            &db::load_duration_map(),
        ) else {
            return;
        };
        self.persist_entity_bar_global(total, global);
    }

    fn shell_needs_dvd_resume_duration_hints(&self, chapter_scrub: bool) -> bool {
        chapter_scrub || self.open_shell_is_chain_head()
    }

    fn file_resume_waits_for_mpv_duration(&self, chapter_scrub: bool) -> bool {
        !self.shell_needs_dvd_resume_duration_hints(chapter_scrub)
    }

    /// Duration to validate the resume against: live mpv, demux kick for scrubs, then DVD hints.
    fn resume_wait_duration(&self, chapter_scrub: bool, pending_t: f64) -> f64 {
        let mut dur = self.finite_positive_duration();
        if dur <= 0.0 && chapter_scrub {
            dur = self.chapter_scrub_demux_duration();
        }
        if dur <= 0.0 {
            dur = self.dvd_hint_duration(chapter_scrub, pending_t);
        }
        dur
    }

    /// DVD duration hints: SQLite duration map first, then `target + 1s` as a working floor.
    fn dvd_hint_duration(&self, chapter_scrub: bool, pending_t: f64) -> f64 {
        if !self.shell_needs_dvd_resume_duration_hints(chapter_scrub) {
            return 0.0;
        }
        if let Some(shell) = self.me_budget_shell_path.borrow().clone() {
            let dur = crate::dvd_vob_timeline::dur_from_map(
                &crate::db::load_duration_map(),
                shell.as_path(),
            );
            if dur > 0.0 {
                return dur;
            }
        }
        if pending_t > 0.0 {
            pending_t + 1.0
        } else {
            0.0
        }
    }

    /// Resume target seconds stashed before **`loadfile`** (consumed by [apply_pending_resume]).
    pub(crate) fn stashed_resume_sec(&self) -> Option<f64> {
        self.pending_resume.get()
    }

    #[must_use]
    pub(crate) fn resume_seek_pending(&self) -> bool {
        self.pending_resume.get().is_some()
    }
}

include!("mpv_bundle_probes.rs");
include!("mpv_pending_resume_apply.rs");
