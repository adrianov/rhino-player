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
        if self
            .me_budget_shell_path
            .borrow()
            .as_ref()
            .is_some_and(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p))
        {
            let hold = self
                .dvd_hold_global
                .get()
                .or_else(|| self.stored_entity_global());
            if let Some(global) = hold {
                let playback = self
                    .mpv
                    .get_property::<f64>("playback-time")
                    .ok()
                    .filter(|t| t.is_finite() && *t >= 0.0)
                    .or_else(|| {
                        self.mpv
                            .get_property::<f64>("time-pos")
                            .ok()
                            .filter(|t| t.is_finite() && *t >= 0.0)
                    })
                    .unwrap_or(0.0);
                self.dvd_chain_bar_sync.set(Some(
                    crate::dvd_vob_timeline::DvdChainBarSync::from_targets(
                        ifo_local,
                        global,
                        playback,
                    ),
                ));
                if let Some(shell) = self.me_budget_shell_path.borrow().clone() {
                    let entity = crate::playback_entity::PlaybackEntity::resolve(&shell);
                    let key = entity.db_path();
                    let map = crate::db::load_duration_map();
                    if let Some(k) = key.to_str() {
                        if let Some(&total) = map.get(k) {
                            if total.is_finite() && total > 0.0 {
                                self.persist_entity_bar_global(total, global);
                            }
                        }
                    }
                }
            }
            self.dvd_hold_global.set(None);
            return;
        }
        self.dvd_hold_global.set(None);
        self.dvd_chain_bar_sync.set(None);
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
        let shell = self.me_budget_shell_path.borrow().clone();
        let chain = shell
            .as_ref()
            .filter(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p));
        let at_target = chain
            .map(|path| resume_seek::resume_already_at_ifo(&self.mpv, path, t))
            .unwrap_or_else(|| resume_seek::resume_already_at(&self.mpv, t));
        if at_target {
            self.clear_pending_resume_done();
            crate::dvd_vob_log::dvd_seek_log(format!(
                "apply_pending_resume: at target {t:.2} (pos={pos:.2})"
            ));
            return Some(t);
        }
        if let Some(path) = chain {
            if !crate::dvd_vob_timeline::chain_head_mpv_ready(path, &self.mpv) {
                crate::dvd_vob_log::dvd_seek_log(format!(
                    "apply_pending_resume: wait chain stretch (target={t:.2})"
                ));
                return Some(t);
            }
            let _ = self.mpv.set_property("pause", false);
            resume_seek::seek_chain_ifo_local(&self.mpv, path, t);
        } else {
            resume_seek::seek_to_resume_sec(&self.mpv, t);
        }
        let at_target = chain
            .map(|path| resume_seek::resume_already_at_ifo(&self.mpv, path, t))
            .unwrap_or_else(|| resume_seek::resume_already_at(&self.mpv, t));
        if at_target {
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

    /// Warm reopen (card click / Space): SQLite fallback when preload cleared pending before seek landed.
    pub fn apply_pending_resume_on_warm_open(&self) -> Option<f64> {
        if !self.mpv_path_matches_shell() || self.pending_resume.get().is_some() {
            return None;
        }
        if !media_probe::mpv_has_known_duration(&self.mpv) {
            return None;
        }
        let t = self.stored_resume_local_for_shell()?;
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
            if !crate::dvd_vob_timeline::chain_head_mpv_ready(path, &self.mpv) {
                return None;
            }
            let _ = self.mpv.set_property("pause", false);
            resume_seek::seek_chain_ifo_local(&self.mpv, path, t);
        } else {
            resume_seek::seek_to_resume_sec(&self.mpv, t);
        }
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

    #[must_use]
    pub(crate) fn resume_seek_pending(&self) -> bool {
        self.pending_resume.get().is_some()
    }
}
