// Cross-chapter DVD pause-hold + resume seek (`impl MpvBundle` extension).

impl MpvBundle {
    /// Pause through cross-chapter `loadfile` until [apply_pending_resume] reaches the target.
    pub(super) fn begin_chapter_scrub_pause_hold(&self, resume_playing: bool) {
        self.chapter_scrub_unpause_after.set(resume_playing);
        self.chapter_scrub_hold_pause.set(true);
        let _ = self.mpv.set_property("pause", true);
        crate::dvd_vob_log::dvd_seek_log(format!(
            "chapter_scrub: pause hold (resume playing={resume_playing})"
        ));
    }

    fn finish_chapter_scrub_pause_hold(&self) {
        if !self.chapter_scrub_hold_pause.replace(false) {
            return;
        }
        let playing = self.chapter_scrub_unpause_after.get();
        let _ = self.mpv.set_property("pause", !playing);
        crate::dvd_vob_log::dvd_seek_log(if playing {
            "chapter_scrub: unpause after resume seek"
        } else {
            "chapter_scrub: re-pause after resume seek"
        });
    }

    /// DVD cross-chapter resume: demux often ignores `seek` while `pause=yes` — unpause for the command.
    fn chapter_scrub_seek_to(&self, ifo_local: f64) {
        if self.chapter_scrub_hold_pause.get() {
            let _ = self.mpv.set_property("pause", false);
        }
        let shell = self.me_budget_shell_path.borrow().clone();
        if let Some(ref path) = shell.filter(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p))
        {
            resume_seek::seek_chain_ifo_local(&self.mpv, path, ifo_local);
        } else {
            resume_seek::seek_to_resume_sec(&self.mpv, ifo_local);
        }
    }

    /// Paused cross-chapter `loadfile` may keep mpv `duration` at 0 until demux runs; kick it.
    pub(super) fn chapter_scrub_demux_duration(&self) -> f64 {
        if self.chapter_scrub_hold_pause.get() {
            let _ = self.mpv.set_property("pause", false);
        }
        let mut dur = self
            .mpv
            .get_property::<f64>("duration")
            .ok()
            .filter(|d| d.is_finite() && *d > 0.0)
            .unwrap_or(0.0);
        if dur <= 0.0 {
            let _ = self.mpv.command("seek", &["0", "absolute"]);
            dur = self
                .mpv
                .get_property::<f64>("duration")
                .ok()
                .filter(|d| d.is_finite() && *d > 0.0)
                .unwrap_or(0.0);
        }
        if dur <= 0.0 {
            dur = self
                .mpv
                .get_property::<f64>("time-pos")
                .ok()
                .filter(|p| p.is_finite() && *p >= 0.0)
                .map(|p| p + 1.0)
                .unwrap_or(0.0);
        }
        dur
    }

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
        let shell = self.me_budget_shell_path.borrow().clone();
        let at_target = if let Some(ref path) =
            shell.filter(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p))
        {
            resume_seek::resume_already_at_ifo(&self.mpv, path, t)
        } else {
            resume_seek::resume_already_at(&self.mpv, t)
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
        if self
            .me_budget_shell_path
            .borrow()
            .as_ref()
            .is_some_and(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p))
        {
            self.dvd_chain_bar_sync
                .set(Some(crate::dvd_vob_timeline::DvdChainBarSync::from_scrub(
                    self, hold, t,
                )));
            self.dvd_hold_global.set(None);
        } else {
            self.dvd_hold_global.set(None);
            self.dvd_chain_bar_sync.set(None);
        }
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

    fn bar_total_from_shell(shell: &Option<std::path::PathBuf>) -> f64 {
        let Some(path) = shell.as_ref() else {
            return 0.0;
        };
        let entity = crate::playback_entity::PlaybackEntity::resolve(path.as_path());
        let key = entity.db_path();
        let map = crate::db::load_duration_map();
        let from_entity = key
            .to_str()
            .and_then(|k| map.get(k).copied())
            .filter(|d| d.is_finite() && *d > 0.0);
        if let Some(d) = from_entity {
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

    pub(crate) fn clear_chapter_scrub_pause_hold(&self) {
        self.chapter_scrub_hold_pause.set(false);
        self.chapter_scrub_unpause_after.set(false);
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

    /// Last-chance unpause when chapter resume retries did not reach the target in time.
    pub(crate) fn force_finish_chapter_scrub_playback(&self) {
        if !self.chapter_scrub_hold_pause.get() && !self.chapter_scrub_resume.get() {
            return;
        }
        let ifo = self.pending_resume.get().unwrap_or(0.0);
        if self.pending_resume.get().is_some() {
            self.chapter_scrub_seek_to(ifo);
        }
        self.pending_resume.set(None);
        self.chapter_scrub_resume.set(false);
        if self
            .me_budget_shell_path
            .borrow()
            .as_ref()
            .is_some_and(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p))
        {
            let hold = self.dvd_hold_global.get().unwrap_or(0.0);
            self.dvd_chain_bar_sync
                .set(Some(crate::dvd_vob_timeline::DvdChainBarSync::from_scrub(
                    self, hold, ifo,
                )));
        } else {
            self.dvd_chain_bar_sync.set(None);
        }
        self.dvd_hold_global.set(None);
        self.finish_chapter_scrub_pause_hold();
    }

    pub(crate) fn clear_chapter_scrub_resume(&self) {
        self.chapter_scrub_resume.set(false);
        self.pending_resume.set(None);
        self.finish_chapter_scrub_pause_hold();
    }

    /// Drop a failed or stale cross-chapter load without unpausing (EOF retry stays at tail).
    pub(crate) fn abort_chapter_load(&self, keep_paused: bool) {
        self.chapter_scrub_resume.set(false);
        self.pending_resume.set(None);
        self.chapter_eof_load.set(false);
        self.dvd_hold_global.set(None);
        self.dvd_chain_bar_sync.set(None);
        self.chapter_scrub_unpause_after.set(!keep_paused);
        self.finish_chapter_scrub_pause_hold();
    }
}
