// Cross-chapter DVD scrub completion (`impl MpvBundle` extension): detect that the resume
// seek landed, seed the chain-bar sync, and persist the title-wide bar total.
// Pause-hold lifecycle lives in `mpv_chapter_scrub_pause_hold.rs`.

impl MpvBundle {
    pub(super) fn apply_chapter_scrub_pending_resume(&self, t: f64) -> Option<f64> {
        if self.complete_chapter_scrub_if_at_target(t) {
            return Some(t);
        }
        self.chapter_scrub_seek_to(t);
        if self.complete_chapter_scrub_if_at_target(t) {
            return Some(t);
        }
        let pos = self.mpv.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
        crate::dvd_vob_log::dvd_seek_log(format!(
            "apply_pending_resume: chapter scrub seek {t:.2} (pos={pos:.2}, retry)"
        ));
        Some(t)
    }

    pub(super) fn complete_chapter_scrub_if_at_target(&self, t: f64) -> bool {
        if !self.chapter_scrub_resume.get() {
            return false;
        }
        let at_target = match &self.chain_head_shell_path() {
            Some(path) => resume_seek::resume_already_at_ifo(&self.mpv, path, t),
            None => resume_seek::resume_already_at(&self.mpv, t),
        };
        if !at_target {
            return false;
        }
        self.finish_chapter_scrub_at_target(t)
    }

    fn finish_chapter_scrub_at_target(&self, t: f64) -> bool {
        let pos = self.mpv.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
        self.pending_resume.set(None);
        self.chapter_scrub_resume.set(false);
        let hold = self.dvd_hold_global.get().unwrap_or(0.0);
        self.finalize_scrub_chain_state(hold, t);
        self.finish_chapter_scrub_pause_hold();
        crate::dvd_vob_log::dvd_seek_log(format!(
            "apply_pending_resume: chapter scrub done target={t:.2} pos={pos:.2}"
        ));
        let total = Self::bar_total_from_shell(&self.me_budget_shell_path.borrow());
        if total > 0.0 {
            self.persist_entity_bar_global(total, hold);
        }
        true
    }

    /// Seed or clear the chain-bar sync from a finished scrub, then release the held global.
    fn finalize_scrub_chain_state(&self, hold: f64, target: f64) {
        if self.open_shell_is_chain_head() {
            self.dvd_chain_bar_sync.set(Some(
                crate::dvd_vob_timeline::DvdChainBarSync::from_scrub(self, hold, target),
            ));
        } else {
            self.dvd_chain_bar_sync.set(None);
        }
        self.dvd_hold_global.set(None);
    }

    fn bar_total_from_shell(shell: &Option<std::path::PathBuf>) -> f64 {
        let Some(path) = shell.as_ref() else {
            return 0.0;
        };
        let key = crate::playback_entity::PlaybackEntity::resolve(path.as_path()).db_path();
        let map = crate::db::load_duration_map();
        if let Some(d) = entity_row_duration(&key, &map) {
            return d;
        }
        crate::dvd_entity::build_title_timeline_with(
            path.as_path(),
            &map,
            crate::dvd_vob_timeline::dur_from_map(&map, path.as_path()),
            crate::dvd_entity::TimelineBuildOpts::CACHE_ONLY,
        )
        .map(|tl| tl.total_sec)
        .unwrap_or(0.0)
    }

    pub(crate) fn clear_chapter_scrub_resume(&self) {
        self.chapter_scrub_resume.set(false);
        self.pending_resume.set(None);
        self.finish_chapter_scrub_pause_hold();
    }

    /// True while a cross-chapter `loadfile` is in flight (pause hold and/or pending resume seek).
    #[must_use]
    pub fn chapter_cross_load_busy(&self) -> bool {
        self.chapter_scrub_hold_pause.get() || self.chapter_scrub_resume_pending()
    }

    /// True while a cross-chapter scrub still needs [apply_pending_resume].
    #[must_use]
    pub fn chapter_scrub_resume_pending(&self) -> bool {
        self.chapter_scrub_resume.get() && self.pending_resume.get().is_some()
    }
}

include!("mpv_chapter_scrub_pause_hold.rs");
