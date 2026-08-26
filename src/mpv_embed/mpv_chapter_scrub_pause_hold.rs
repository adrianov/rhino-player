// Cross-chapter DVD pause-hold lifecycle (`impl MpvBundle` extension): hold `pause` through
// the `loadfile`, kick demux for a duration, and force-finish when retries run out.
// Scrub completion logic lives in `mpv_chapter_scrub.rs`.

impl MpvBundle {
    /// Pause through cross-chapter `loadfile` until [apply_pending_resume] reaches the target.
    pub(super) fn begin_chapter_scrub_pause_hold(&self, resume_playing: bool) {
        self.chapter_scrub_unpause_after.set(resume_playing);
        self.chapter_scrub_hold_pause.set(true);
        if resume_playing {
            crate::screen_blackout::begin_tech_hold();
        }
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
        if playing {
            crate::screen_blackout::end_tech_hold();
        }
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
        match &self.chain_head_shell_path() {
            Some(path) => resume_seek::seek_chain_ifo_local(&self.mpv, path, ifo_local),
            None => resume_seek::seek_to_resume_sec(&self.mpv, ifo_local),
        }
    }

    /// Paused cross-chapter `loadfile` may keep mpv `duration` at 0 until demux runs; kick it.
    pub(super) fn chapter_scrub_demux_duration(&self) -> f64 {
        if self.chapter_scrub_hold_pause.get() {
            let _ = self.mpv.set_property("pause", false);
        }
        let mut dur = self.finite_positive_duration();
        if dur <= 0.0 {
            let _ = self.mpv.command("seek", &["0", "absolute"]);
            dur = self.finite_positive_duration();
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
        let hold = self.dvd_hold_global.get().unwrap_or(0.0);
        self.finalize_scrub_chain_state(hold, ifo);
        self.finish_chapter_scrub_pause_hold();
    }

    pub(crate) fn clear_chapter_scrub_pause_hold(&self) {
        if self.chapter_scrub_hold_pause.get() && self.chapter_scrub_unpause_after.get() {
            crate::screen_blackout::end_tech_hold();
        }
        self.chapter_scrub_hold_pause.set(false);
        self.chapter_scrub_unpause_after.set(false);
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
