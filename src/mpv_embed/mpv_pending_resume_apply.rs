// Pending-resume application: wait policy, seek execution, and settle/clear after the seek
// (`impl MpvBundle` extension). Lookups + duration policy live in `mpv_pending_resume.rs`.

impl MpvBundle {
    /// Open mpv `path` matches [Self::set_me_budget_shell_path] (set before `loadfile`).
    fn mpv_path_matches_shell(&self) -> bool {
        let shell = self.me_budget_shell_path.borrow();
        let Some(target) = shell.as_ref() else {
            return true;
        };
        media_probe::mpv_matches_open_target(&self.mpv, shell.as_deref(), target.as_path())
    }
    fn drop_stale_pending_state(&self) -> Option<f64> {
        self.dvd_hold_global.set(None);
        if self.dvd_chain_bar_sync.get().is_none() {
            self.dvd_chain_bar_sync.set(None);
        }
        self.chapter_scrub_resume.set(false);
        if self.chapter_scrub_hold_pause.get() {
            self.finish_chapter_scrub_pause_hold();
        }
        None
    }

    /// `FileLoaded` not visible yet: mpv `path` still trails the shell target.
    fn wait_for_mpv_path(&self, t: f64) -> Option<f64> {
        crate::dvd_vob_log::resume_open_log(format!(
            "apply wait mpv path local={t:.2} shell={:?}",
            self.me_budget_shell_path.borrow().as_deref()
        ));
        crate::dvd_vob_log::dvd_seek_log(format!(
            "apply_pending_resume: wait path (target={t:.2})"
        ));
        None
    }

    /// Duration not known yet: cannot validate the resume target.
    fn wait_for_known_duration(&self, chapter_scrub: bool, pending_t: f64) -> Option<f64> {
        crate::dvd_vob_log::resume_open_log(format!(
            "apply wait duration local={pending_t:.2} scrub={chapter_scrub}"
        ));
        crate::dvd_vob_log::dvd_seek_log(format!(
            "apply_pending_resume: wait duration (target={pending_t:.2})"
        ));
        None
    }

    /// Plain-file resume waits for demux to publish a duration before seeking.
    fn wait_for_demux(&self, pending_t: f64) -> Option<f64> {
        crate::dvd_vob_log::resume_open_log(format!("apply wait demux local={pending_t:.2}"));
        crate::dvd_vob_log::dvd_seek_log(format!(
            "apply_pending_resume: wait demux (target={pending_t:.2})"
        ));
        None
    }

    /// Apply the resume stashed by the most recent [load_file_path] or [load_chapter_seek].
    pub fn apply_pending_resume(&self) -> Option<f64> {
        let Some(t) = self.pending_resume.get() else {
            return self.drop_stale_pending_state();
        };
        if !self.mpv_path_matches_shell() {
            return self.wait_for_mpv_path(t);
        }
        let chapter_scrub = self.chapter_scrub_resume.get();
        if self.resume_wait_duration(chapter_scrub, t) <= 0.0 {
            return self.wait_for_known_duration(chapter_scrub, t);
        }
        if chapter_scrub {
            return self.apply_chapter_scrub_pending_resume(t);
        }
        if self.file_resume_waits_for_mpv_duration(chapter_scrub)
            && !media_probe::mpv_has_known_duration(&self.mpv)
        {
            return self.wait_for_demux(t);
        }
        self.apply_file_pending_resume()
    }

    /// Apply a plain-file (or chain-head) stashed resume once its target is reachable.
    fn apply_file_pending_resume(&self) -> Option<f64> {
        if let Some(p) = self.persist_media_path() {
            resume_seek::stash_near_start_resume(&self.mpv, &self.pending_resume, &p);
        }
        let t = self.pending_resume.get()?;
        let pos = self.mpv.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
        let mpv_dur = self.finite_duration_secs();
        if self.resume_at_target(t) {
            self.clear_pending_resume_done();
            self.log_resume_at_target(t, pos, mpv_dur);
            return Some(t);
        }
        self.seek_and_settle_resume(t, pos, mpv_dur);
        Some(t)
    }

    /// Seek toward the pending resume target and settle state when it already landed.
    /// Chain-head loads unpause for the command — demux often ignores `seek` while paused.
    fn seek_and_settle_resume(&self, t: f64, pos: f64, mpv_dur: f64) {
        let chain = self.chain_head_shell_path();
        match chain.as_deref() {
            Some(path) => self.seek_chain_head_resume(path, t, pos, mpv_dur),
            None => resume_seek::seek_to_resume_sec(&self.mpv, t),
        }
        if self.resume_at_target(t) {
            self.clear_pending_resume_done();
        }
        if chain.is_none() {
            crate::dvd_vob_log::resume_open_log(format!(
                "apply seek local={t:.2} pos={pos:.2} dur={mpv_dur:.2}"
            ));
        }
        crate::dvd_vob_log::dvd_seek_log(format!(
            "apply_pending_resume: seek {t:.2} (was pos={pos:.2})"
        ));
    }

    /// Chain-head seek: unpause, remap IFO-local seconds onto the stretched mpv timeline.
    fn seek_chain_head_resume(&self, path: &std::path::Path, t: f64, pos: f64, mpv_dur: f64) {
        let _ = self.mpv.set_property("pause", false);
        let seg = crate::dvd_vob_timeline::chain_head_ifo_seg(path).unwrap_or(t);
        let mpv_t = crate::dvd_vob_timeline::chain_head_mpv_seek_sec(&self.mpv, t, seg);
        resume_seek::seek_chain_ifo_local(&self.mpv, path, t);
        crate::dvd_vob_log::resume_open_log(format!(
            "apply chain seek ifo={t:.2} -> mpv={mpv_t:.2} pos={pos:.2} dur={mpv_dur:.2} stretched={}",
            crate::dvd_vob_timeline::chain_head_stretched(mpv_dur, seg)
        ));
    }

    fn log_resume_at_target(&self, t: f64, pos: f64, mpv_dur: f64) {
        crate::dvd_vob_log::resume_open_log(format!(
            "apply at target local={t:.2} pos={pos:.2} dur={mpv_dur:.2}"
        ));
        crate::dvd_vob_log::dvd_seek_log(format!(
            "apply_pending_resume: at target {t:.2} (pos={pos:.2})"
        ));
    }

    /// Warm reopen (card click / Space): SQLite fallback when preload cleared pending before seek landed.
    pub fn apply_pending_resume_on_warm_open(&self) -> Option<f64> {
        if !self.mpv_path_matches_shell() || self.pending_resume.get().is_some() {
            return None;
        }
        if !media_probe::mpv_has_known_duration(&self.mpv) {
            crate::dvd_vob_log::resume_open_log("warm_open wait duration");
            return None;
        }
        let Some(t) = self.stored_resume_local_for_shell() else {
            crate::dvd_vob_log::resume_open_log("warm_open no stored local for shell");
            return None;
        };
        if self.resume_at_target(t) {
            return Some(t);
        }
        self.seek_warm_open_target(t);
        crate::dvd_vob_log::resume_open_log(format!("warm_open seek local={t:.2}"));
        Some(t)
    }

    /// Warm-open seek: chain-head IFO coords (unpaused for the command) or plain seconds.
    fn seek_warm_open_target(&self, t: f64) {
        match &self.chain_head_shell_path() {
            Some(path) => {
                let _ = self.mpv.set_property("pause", false);
                resume_seek::seek_chain_ifo_local(&self.mpv, path, t);
            }
            None => resume_seek::seek_to_resume_sec(&self.mpv, t),
        }
    }

    /// Continue-grid reveal / card open: apply stashed or SQLite resume before unpausing.
    pub fn ensure_resume_before_unpause(&self) -> Option<f64> {
        let pending = self.pending_resume.get();
        let hold = self.dvd_hold_global.get();
        if let Some(t) = self.apply_pending_resume() {
            crate::dvd_vob_log::resume_open_log(format!(
                "ensure ok local={t:.2} pending_before={pending:?} hold={hold:?}"
            ));
            return Some(t);
        }
        if self.pending_resume.get().is_some() {
            crate::dvd_vob_log::resume_open_log(format!(
                "ensure deferred pending={pending:?} hold={hold:?}"
            ));
            return None;
        }
        let warm = self.apply_pending_resume_on_warm_open();
        crate::dvd_vob_log::resume_open_log(format!(
            "ensure warm_open={} hold={hold:?}",
            warm.map(|t| format!("{t:.2}"))
                .unwrap_or_else(|| "none".into())
        ));
        warm
    }
}
