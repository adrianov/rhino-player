// Pending resume + DVD chain-bar handoff after chapter scrub (`impl MpvBundle` extension).

impl MpvBundle {
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

    fn stored_entity_global(&self) -> Option<f64> {
        let shell = self.me_budget_shell_path.borrow().clone()?;
        let entity = crate::playback_entity::PlaybackEntity::resolve(&shell);
        crate::db::resume_pos(&entity.db_path())
    }

    fn clear_pending_resume_done(&self) {
        let ifo_local = self.pending_resume.get().unwrap_or(0.0);
        self.pending_resume.set(None);
        let is_chain_head = self
            .me_budget_shell_path
            .borrow()
            .as_ref()
            .is_some_and(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p));
        if !is_chain_head {
            self.dvd_hold_global.set(None);
            self.dvd_chain_bar_sync.set(None);
            return;
        }
        let hold = self
            .dvd_hold_global
            .get()
            .or_else(|| self.stored_entity_global());
        if let Some(global) = hold {
            let playback = self.current_playback_seconds();
            self.dvd_chain_bar_sync.set(Some(
                crate::dvd_vob_timeline::DvdChainBarSync::from_targets(ifo_local, global, playback),
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
        let entity = crate::playback_entity::PlaybackEntity::resolve(&shell);
        let key = entity.db_path();
        let Some(k) = key.to_str() else {
            return;
        };
        let map = crate::db::load_duration_map();
        let Some(&total) = map.get(k) else {
            return;
        };
        if total.is_finite() && total > 0.0 {
            self.persist_entity_bar_global(total, global);
        }
    }

    fn shell_needs_dvd_resume_duration_hints(&self, chapter_scrub: bool) -> bool {
        chapter_scrub
            || self
                .me_budget_shell_path
                .borrow()
                .as_ref()
                .is_some_and(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p))
    }

    fn file_resume_waits_for_mpv_duration(&self, chapter_scrub: bool) -> bool {
        !self.shell_needs_dvd_resume_duration_hints(chapter_scrub)
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
        if dur <= 0.0 && self.shell_needs_dvd_resume_duration_hints(chapter_scrub) {
            if let Some(shell) = self.me_budget_shell_path.borrow().clone() {
                dur = crate::dvd_vob_timeline::dur_from_map(
                    &crate::db::load_duration_map(),
                    shell.as_path(),
                );
            }
        }
        if dur <= 0.0 && pending_t > 0.0 && self.shell_needs_dvd_resume_duration_hints(chapter_scrub) {
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
        let mpv_dur = self
            .mpv
            .get_property::<f64>("duration")
            .ok()
            .filter(|d| d.is_finite())
            .unwrap_or(0.0);
        let shell = self.me_budget_shell_path.borrow().clone();
        let chain = shell
            .as_ref()
            .filter(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p));
        let at_target = chain
            .map(|path| resume_seek::resume_already_at_ifo(&self.mpv, path, t))
            .unwrap_or_else(|| resume_seek::resume_already_at(&self.mpv, t));
        if at_target {
            self.clear_pending_resume_done();
            crate::dvd_vob_log::resume_open_log(format!(
                "apply at target local={t:.2} pos={pos:.2} dur={mpv_dur:.2}"
            ));
            crate::dvd_vob_log::dvd_seek_log(format!(
                "apply_pending_resume: at target {t:.2} (pos={pos:.2})"
            ));
            return Some(t);
        }
        if let Some(path) = chain {
            let _ = self.mpv.set_property("pause", false);
            let seg = crate::dvd_vob_timeline::chain_head_ifo_seg(path).unwrap_or(t);
            let mpv_t = crate::dvd_vob_timeline::chain_head_mpv_seek_sec(&self.mpv, t, seg);
            resume_seek::seek_chain_ifo_local(&self.mpv, path, t);
            crate::dvd_vob_log::resume_open_log(format!(
                "apply chain seek ifo={t:.2} -> mpv={mpv_t:.2} pos={pos:.2} dur={mpv_dur:.2} stretched={}",
                crate::dvd_vob_timeline::chain_head_stretched(mpv_dur, seg)
            ));
        } else {
            resume_seek::seek_to_resume_sec(&self.mpv, t);
        }
        let at_target = chain
            .map(|path| resume_seek::resume_already_at_ifo(&self.mpv, path, t))
            .unwrap_or_else(|| resume_seek::resume_already_at(&self.mpv, t));
        if at_target {
            self.clear_pending_resume_done();
        }
        if chain.is_none() {
            crate::dvd_vob_log::resume_open_log(format!(
                "apply seek local={t:.2} pos={pos:.2} dur={mpv_dur:.2}"
            ));
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

    /// Apply the resume stashed by the most recent [load_file_path] or [load_chapter_seek].
    pub fn apply_pending_resume(&self) -> Option<f64> {
        let Some(t) = self.pending_resume.get() else {
            self.dvd_hold_global.set(None);
            if self.dvd_chain_bar_sync.get().is_none() {
                self.dvd_chain_bar_sync.set(None);
            }
            self.chapter_scrub_resume.set(false);
            if self.chapter_scrub_hold_pause.get() {
                self.finish_chapter_scrub_pause_hold();
            }
            return None;
        };
        if !self.mpv_path_matches_shell() {
            crate::dvd_vob_log::resume_open_log(format!(
                "apply wait mpv path local={t:.2} shell={:?}",
                self.me_budget_shell_path.borrow().as_deref()
            ));
            crate::dvd_vob_log::dvd_seek_log(format!(
                "apply_pending_resume: wait path (target={t:.2})"
            ));
            return None;
        }
        let chapter_scrub = self.chapter_scrub_resume.get();
        let pending_t = self.pending_resume.get().unwrap_or(0.0);
        let dur = self.resume_wait_duration(chapter_scrub, pending_t);
        if dur <= 0.0 {
            crate::dvd_vob_log::resume_open_log(format!(
                "apply wait duration local={pending_t:.2} scrub={chapter_scrub}"
            ));
            crate::dvd_vob_log::dvd_seek_log(format!(
                "apply_pending_resume: wait duration (target={pending_t:.2})"
            ));
            return None;
        }
        let t = self.pending_resume.get()?;
        if chapter_scrub {
            return self.apply_chapter_scrub_pending_resume(t);
        }
        if self.file_resume_waits_for_mpv_duration(chapter_scrub)
            && !media_probe::mpv_has_known_duration(&self.mpv)
        {
            crate::dvd_vob_log::resume_open_log(format!(
                "apply wait demux local={pending_t:.2}"
            ));
            crate::dvd_vob_log::dvd_seek_log(format!(
                "apply_pending_resume: wait demux (target={pending_t:.2})"
            ));
            return None;
        }
        self.apply_file_pending_resume()
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
        let shell = self.me_budget_shell_path.borrow().clone();
        let chain = shell
            .as_ref()
            .filter(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p));
        let at_target = chain
            .map(|path| resume_seek::resume_already_at_ifo(&self.mpv, path, t))
            .unwrap_or_else(|| resume_seek::resume_already_at(&self.mpv, t));
        if at_target {
            return Some(t);
        }
        if let Some(path) = chain {
            let _ = self.mpv.set_property("pause", false);
            resume_seek::seek_chain_ifo_local(&self.mpv, path, t);
        } else {
            resume_seek::seek_to_resume_sec(&self.mpv, t);
        }
        crate::dvd_vob_log::resume_open_log(format!("warm_open seek local={t:.2}"));
        Some(t)
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

    #[must_use]
    pub(crate) fn resume_seek_pending(&self) -> bool {
        self.pending_resume.get().is_some()
    }
}
