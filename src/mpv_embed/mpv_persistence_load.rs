// `MpvBundle` load targets: resolve what to `loadfile` next, hand off the outgoing item's
// resume, and reset per-open state. Included at module level from `mpv_persistence.rs`.

impl MpvBundle {
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
        let (entity, canonical, unified) = self.handoff_outgoing_entity(
            path,
            clear_outgoing_resume,
            snapshot_outgoing,
            warm_preload,
        );
        let db_key = entity.db_path();
        let (load_path, pending, stored) =
            self.resolve_load_target(&entity, &canonical, &db_key, resume_at);
        self.log_resolved_load_target(unified, &load_path, &db_key, stored, pending);
        self.reset_preview_if_entity_changed_from(&load_path, "load_file_path:entity_change");
        let s = load_path.to_str().ok_or("media path is not valid UTF-8")?;
        self.prepare_next_load_state(unified, &load_path, pending, stored);
        self.mpv
            .command("loadfile", &[s, "replace"])
            .map_err(|e| format!("{e:?}"))
    }

    /// Canonicalize the incoming path, hand off the outgoing item's resume, and sanitize stale
    /// unified-entity rows. Returns the resolved entity plus canonical path / unified flag.
    fn handoff_outgoing_entity(
        &self,
        path: &Path,
        clear_outgoing_resume: bool,
        snapshot_outgoing: bool,
        warm_preload: bool,
    ) -> (
        crate::playback_entity::PlaybackEntity,
        std::path::PathBuf,
        bool,
    ) {
        let canonical = canonicalize_media_path(path);
        let entity = crate::playback_entity::PlaybackEntity::resolve(&canonical);
        let db_key = entity.db_path();
        let unified = entity.has_unified_timeline();
        self.handle_outgoing_resume(
            &db_key,
            unified,
            clear_outgoing_resume,
            snapshot_outgoing,
            warm_preload,
        );
        if unified {
            crate::dvd_entity::sanitize_stale_entity_playback(&canonical, 0.0);
        }
        (entity, canonical, unified)
    }

    fn log_resolved_load_target(
        &self,
        unified: bool,
        load_path: &Path,
        db_key: &Path,
        stored: Option<f64>,
        pending: Option<f64>,
    ) {
        if !unified {
            return;
        }
        crate::dvd_vob_log::resume_open_log(format!(
            "load global={stored:?} local={pending:?} file={} entity={}",
            load_path.display(),
            db_key.display()
        ));
    }

    /// Reset per-open state and stash the resume targets for the imminent `loadfile`.
    fn prepare_next_load_state(
        &self,
        unified: bool,
        load_path: &Path,
        pending: Option<f64>,
        stored: Option<f64>,
    ) {
        self.warm_file_gen
            .set(self.warm_file_gen.get().wrapping_add(1));
        if unified {
            crate::dvd_vob_log::resume_open_log(format!(
                "load stashed pending={pending:?} hold={stored:?} gen={}",
                self.warm_file_gen.get()
            ));
        }
        self.clear_chapter_scrub_pause_hold();
        self.clear_smooth_vf_stripped_this_open();
        self.clear_smooth_vf_reload_attempted();
        self.chapter_scrub_resume.set(false);
        self.dvd_chain_bar_sync.set(None);
        self.dvd_hold_global
            .set(if unified { stored } else { None });
        self.pending_resume.set(pending);
        self.set_me_budget_shell_path(load_path);
    }

    /// Snapshot or clear the outgoing item's resume before `loadfile` replaces it.
    fn handle_outgoing_resume(
        &self,
        db_key: &Path,
        unified: bool,
        clear_outgoing_resume: bool,
        snapshot_outgoing: bool,
        warm_preload: bool,
    ) {
        let shell = self.me_budget_shell_path.borrow();
        let outgoing = media_probe::shell_media_path(&self.mpv, shell.as_deref());
        let same_entity = Self::outgoing_paths_same_entity(outgoing.as_deref(), db_key);
        if clear_outgoing_resume && !warm_preload {
            if let Some(p) = outgoing.as_ref() {
                media_probe::clear_resume_for_path(p);
            }
        } else if snapshot_outgoing && !warm_preload && !same_entity {
            self.save_playback_state_for_close();
        }
        self.log_outgoing_resume(
            unified,
            outgoing.as_deref(),
            same_entity,
            snapshot_outgoing,
            warm_preload,
        );
    }

    /// True when the outgoing media resolves to the same entity row as the incoming one.
    fn outgoing_paths_same_entity(outgoing: Option<&Path>, db_key: &Path) -> bool {
        outgoing.is_some_and(|p| {
            crate::video_ext::paths_same_file(
                &crate::playback_entity::PlaybackEntity::resolve(p).db_path(),
                db_key,
            )
        })
    }

    fn log_outgoing_resume(
        &self,
        unified: bool,
        outgoing: Option<&Path>,
        same_entity: bool,
        snapshot_outgoing: bool,
        warm_preload: bool,
    ) {
        if !unified {
            return;
        }
        let disp = outgoing
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "?".into());
        crate::dvd_vob_log::resume_open_log(format!(
        "load outgoing={disp} same_entity={same_entity} snapshot={snapshot_outgoing} warm={warm_preload}"
    ));
    }
}
