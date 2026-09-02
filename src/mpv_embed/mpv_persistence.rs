// `MpvBundle` persistence + load methods. Split out of `main_bundle_egl_render.rs` so the
// platform-shaped construction code stays focused. `include!`'d at module level so it
// extends `MpvBundle` with another `impl` block (Rust forbids `include!` inside an impl).
// Entity-row writes live in `mpv_persistence_entity_rows.rs`, load targets in
// `mpv_persistence_load.rs`.

impl MpvBundle {
    fn persist_media_path(&self) -> Option<std::path::PathBuf> {
        media_probe::shell_media_path(&self.mpv, self.me_budget_shell_path.borrow().as_deref())
    }

    /// Remember [Path] the shell just opened for ME budget + **`media`** row lookup (not read from mpv).
    pub(crate) fn set_me_budget_shell_path(&self, path: &Path) {
        *self.me_budget_shell_path.borrow_mut() =
            Some(std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));
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

    /// A usable title-wide bar pair: finite total > 0 and finite global >= 0.
    fn valid_bar_pair(total: f64, global: f64) -> bool {
        total.is_finite() && total > 0.0 && global.is_finite() && global >= 0.0
    }

    /// Remember title-wide bar position for SQLite entity rows (chain-head mpv coords are not global).
    pub(crate) fn set_transport_bar_persist(&self, total: f64, global: f64) {
        if Self::valid_bar_pair(total, global) {
            self.transport_bar_total.set(Some(total));
            self.transport_bar_global.set(Some(global));
        }
    }

    pub(crate) fn clear_transport_bar_persist(&self) {
        self.transport_bar_total.set(None);
        self.transport_bar_global.set(None);
    }

    fn transport_persist_pair(&self) -> Option<(f64, f64)> {
        match (
            self.transport_bar_total.get(),
            self.transport_bar_global.get(),
        ) {
            (Some(t), Some(g)) if Self::valid_bar_pair(t, g) => Some((t, g)),
            _ => None,
        }
    }

    /// Reset the seek-bar preview when the incoming file belongs to a different entity than the
    /// previously open one (`from` names the caller in the debug log).
    fn reset_preview_if_entity_changed_from(&self, load_path: &Path, from: &'static str) {
        let prev_shell = self.me_budget_shell_path.borrow().clone();
        if prev_shell
            .as_ref()
            .is_some_and(|p| crate::preview_debug::open_target_entity_changed(p, load_path))
        {
            crate::seek_bar_preview::reset_on_main_media_change_from(from);
        }
    }

    fn snapshot_playback_inner(&self) {
        media_probe::record_playback_for_current(
            &self.mpv,
            self.me_budget_shell_path.borrow().as_deref(),
            self.transport_persist_pair(),
        );
    }

    /// End playback; call after the SQLite snapshot. Safe to skip before process exit.
    pub fn stop_playback(&self) {
        crate::seek_bar_preview::reset_on_main_media_change_from("stop_playback");
        *self.me_budget_shell_path.borrow_mut() = None;
        self.clear_transport_bar_persist();
        let _ = self.mpv.command("stop", &[]);
    }

    /// Close / quit / back-from-playback: persist the open file unless the shell asked to skip
    /// (warm preload, or playing-file trash — Finder recycle pumps the main loop).
    pub fn save_playback_state_for_close(&self) {
        self.save_playback_state_for_close_with_bar(None);
    }

    /// Browse-back / quit: map live mpv + DVD bar to entity-global resume before the grid reads SQLite.
    pub fn save_playback_state_for_close_with_bar(
        &self,
        bar: Option<&crate::dvd_vob_timeline::DvdBarState>,
    ) {
        if !self.may_persist_media_rows() {
            return;
        }
        let Some((total, global)) = self.entity_bar_snapshot_now(bar) else {
            self.snapshot_playback_inner();
            return;
        };
        if !(total > 0.0 && global.is_finite()) {
            self.snapshot_playback_inner();
            return;
        }
        let shell = self.me_budget_shell_path.borrow().clone();
        let unified = shell.as_ref().is_some_and(|p| {
            crate::playback_entity::PlaybackEntity::resolve(p).has_unified_timeline()
        });
        if unified {
            self.write_entity_playback(total, global);
            return;
        }
        self.snapshot_playback_inner();
    }

    /// Save SQLite resume snapshot, then stop playback. Used at process quit.
    pub fn commit_quit(&self) {
        self.save_playback_state_for_close();
        self.stop_playback();
    }
    /// Resolve the file to load plus any pending-resume local seconds and the stored global, from an
    /// explicit `resume_at`, the SQLite resume, or (unified timelines) the IFO movie entry.
    fn resolve_load_target(
        &self,
        entity: &crate::playback_entity::PlaybackEntity,
        canonical: &Path,
        db_key: &Path,
        resume_at: Option<f64>,
    ) -> (std::path::PathBuf, Option<f64>, Option<f64>) {
        let unified = entity.has_unified_timeline();
        let Some(global) = Self::stored_resume_global(db_key, resume_at, unified) else {
            if unified {
                crate::dvd_vob_log::resume_open_log(format!(
                    "load no stored resume entity={}",
                    db_key.display()
                ));
            }
            return (canonical.to_path_buf(), None, None);
        };
        let map = db::load_duration_map();
        match entity.resume_load_target(canonical, global, &map) {
            Some((target, local)) => (target, Some(local), Some(global)),
            None => {
                crate::dvd_vob_log::resume_open_log(format!(
                    "load resume_load_target failed global={global:.2} probe={}",
                    canonical.display()
                ));
                (canonical.to_path_buf(), None, Some(global))
            }
        }
    }

    /// Explicit `resume_at`, else SQLite resume, else (unified timelines) the IFO movie entry global.
    fn stored_resume_global(db_key: &Path, resume_at: Option<f64>, unified: bool) -> Option<f64> {
        resume_at.or_else(|| db::resume_pos(db_key)).or_else(|| {
            unified
                .then(|| crate::dvd_ifo_parse::movie_entry_global_sec(db_key))
                .flatten()
        })
    }
}

include!("mpv_persistence_entity_rows.rs");
include!("mpv_persistence_load.rs");
include!("mpv_persistence_chapter_seek.rs");
