// `MpvBundle` entity-row writes + title-wide-bar snapshots (SQLite `playback` / unified
// DVD entity rows). Included at module level from `mpv_persistence.rs`.

impl MpvBundle {
    /// Write title-wide bar position into the entity SQLite row (continue grid / resume).
    pub(crate) fn persist_entity_bar_global(&self, total: f64, global: f64) {
        self.set_transport_bar_persist(total, global);
        self.write_entity_playback(total, global);
    }

    fn entity_title_total_sec(&self) -> Option<f64> {
        let shell = self.me_budget_shell_path.borrow().clone()?;
        entity_row_duration(
            &crate::playback_entity::PlaybackEntity::resolve(&shell).db_path(),
            &db::load_duration_map(),
        )
    }

    /// Snapshot to persist now: transport bar pair, held DVD global, or a live unified-timeline
    /// mapping of the open media — first available wins.
    fn entity_bar_snapshot_now(
        &self,
        bar: Option<&crate::dvd_vob_timeline::DvdBarState>,
    ) -> Option<(f64, f64)> {
        if let Some(pair) = self.transport_persist_pair() {
            return Some(pair);
        }
        if let Some(pair) = self.hold_global_bar_snapshot(bar) {
            return Some(pair);
        }
        self.live_entity_transport_bar(bar)
    }

    /// Snapshot during a pinned cross-chapter hold: `(title total, held global)`.
    fn hold_global_bar_snapshot(
        &self,
        bar: Option<&crate::dvd_vob_timeline::DvdBarState>,
    ) -> Option<(f64, f64)> {
        let h = self.dvd_hold_global.get()?;
        let total = bar
            .map(crate::dvd_vob_timeline::DvdBarState::total_sec)
            .filter(|t| *t > 0.0)
            .or_else(|| self.entity_title_total_sec())?;
        Some((total, h))
    }

    /// Live unified-timeline mapping of the open media onto the title-wide bar.
    fn live_entity_transport_bar(
        &self,
        bar: Option<&crate::dvd_vob_timeline::DvdBarState>,
    ) -> Option<(f64, f64)> {
        let shell = self.me_budget_shell_path.borrow().clone();
        let chapter = media_probe::shell_media_path(&self.mpv, shell.as_deref())?;
        let entity = crate::playback_entity::PlaybackEntity::resolve(&chapter);
        if !entity.has_unified_timeline() {
            return None;
        }
        let pos = self.finite_mpv_secs_nonneg("time-pos")?;
        let dur = self.finite_mpv_secs_nonneg("duration")?;
        Some(entity.transport_bar(&chapter, pos, dur, bar, Some(self)))
    }

    fn write_entity_playback(&self, total: f64, global: f64) {
        if !Self::valid_bar_pair(total, global) {
            return;
        }
        let shell = self.me_budget_shell_path.borrow().clone();
        let Some(chapter) = media_probe::shell_media_path(&self.mpv, shell.as_deref()) else {
            return;
        };
        let entity = crate::playback_entity::PlaybackEntity::resolve(&chapter);
        if entity.has_unified_timeline() {
            self.save_unified_entity_global(&entity, total, global);
        } else {
            crate::db::set_playback(&entity.db_path(), total, global);
            entity.purge_extra_db_rows();
            crate::media_probe::continue_grid_cache_note_playback(&entity.db_path(), global, total);
        }
        crate::dvd_vob_log::dvd_seek_log(format!(
            "persist entity global={global:.2} total={total:.1} ({})",
            entity.db_path().display()
        ));
    }

    /// Unified timelines store the resume on the entity row itself (not the plain media table).
    fn save_unified_entity_global(
        &self,
        entity: &crate::playback_entity::PlaybackEntity,
        total: f64,
        global: f64,
    ) {
        entity.save_global_resume(total, global);
        crate::dvd_vob_log::resume_open_log(format!(
            "save entity global={global:.2} total={total:.1} ({})",
            entity.db_path().display()
        ));
    }
}
