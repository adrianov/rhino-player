// `MpvBundle` persistence + load methods. Split out of `main_bundle_egl_render.rs` so the
// platform-shaped construction code stays focused. `include!`'d at module level so it
// extends `MpvBundle` with another `impl` block (Rust forbids `include!` inside an impl).

impl MpvBundle {

fn persist_media_path(&self) -> Option<std::path::PathBuf> {
    media_probe::shell_media_path(
        &self.mpv,
        self.me_budget_shell_path.borrow().as_deref(),
    )
}

/// Remember [Path] the shell just opened for ME budget + **`media`** row lookup (not read from mpv).
pub(crate) fn set_me_budget_shell_path(&self, path: &Path) {
    *self.me_budget_shell_path.borrow_mut() = Some(
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
    );
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

/// Remember title-wide bar position for SQLite entity rows (chain-head mpv coords are not global).
pub(crate) fn set_transport_bar_persist(&self, total: f64, global: f64) {
    if total.is_finite() && total > 0.0 && global.is_finite() && global >= 0.0 {
        self.transport_bar_total.set(Some(total));
        self.transport_bar_global.set(Some(global));
    }
}

pub(crate) fn clear_transport_bar_persist(&self) {
    self.transport_bar_total.set(None);
    self.transport_bar_global.set(None);
}

/// Write title-wide bar position into the entity SQLite row (continue grid / resume).
pub(crate) fn persist_entity_bar_global(&self, total: f64, global: f64) {
    self.set_transport_bar_persist(total, global);
    self.write_entity_playback(total, global);
}

fn entity_title_total_sec(&self) -> Option<f64> {
    let shell = self.me_budget_shell_path.borrow().clone()?;
    let entity = crate::playback_entity::PlaybackEntity::resolve(&shell);
    let key = entity.db_path();
    let map = crate::db::load_duration_map();
    key.to_str()
        .and_then(|k| map.get(k).copied())
        .filter(|d| d.is_finite() && *d > 0.0)
}

fn entity_bar_snapshot_now(
    &self,
    bar: Option<&crate::dvd_vob_timeline::DvdBarState>,
) -> Option<(f64, f64)> {
    if let Some(pair) = self.transport_persist_pair() {
        return Some(pair);
    }
    if let Some(h) = self.dvd_hold_global.get() {
        let total = bar
            .map(crate::dvd_vob_timeline::DvdBarState::total_sec)
            .filter(|t| *t > 0.0)
            .or_else(|| self.entity_title_total_sec())?;
        return Some((total, h));
    }
    let shell = self.me_budget_shell_path.borrow().clone();
    let chapter = media_probe::shell_media_path(&self.mpv, shell.as_deref())?;
    let entity = crate::playback_entity::PlaybackEntity::resolve(&chapter);
    if !entity.has_unified_timeline() {
        return None;
    }
    let pos = self
        .mpv
        .get_property::<f64>("time-pos")
        .ok()
        .filter(|p| p.is_finite())?
        .max(0.0);
    let dur = self
        .mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite())?
        .max(0.0);
    Some(entity.transport_bar(&chapter, pos, dur, bar, Some(self)))
}

fn write_entity_playback(&self, total: f64, global: f64) {
    if !(total.is_finite() && total > 0.0 && global.is_finite() && global >= 0.0) {
        return;
    }
    let shell = self.me_budget_shell_path.borrow().clone();
    let Some(chapter) = media_probe::shell_media_path(&self.mpv, shell.as_deref()) else {
        return;
    };
    let entity = crate::playback_entity::PlaybackEntity::resolve(&chapter);
    if entity.has_unified_timeline() {
        entity.save_global_resume(total, global);
        crate::dvd_vob_log::resume_open_log(format!(
            "save entity global={global:.2} total={total:.1} ({})",
            entity.db_path().display()
        ));
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

fn transport_persist_pair(&self) -> Option<(f64, f64)> {
    match (
        self.transport_bar_total.get(),
        self.transport_bar_global.get(),
    ) {
        (Some(t), Some(g)) if t.is_finite() && t > 0.0 && g.is_finite() && g >= 0.0 => Some((t, g)),
        _ => None,
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

/// Close / quit / back-from-playback: always persist the open file.
pub fn save_playback_state_for_close(&self) {
    self.save_playback_state_for_close_with_bar(None);
}

/// Browse-back / quit: map live mpv + DVD bar to entity-global resume before the grid reads SQLite.
pub fn save_playback_state_for_close_with_bar(
    &self,
    bar: Option<&crate::dvd_vob_timeline::DvdBarState>,
) {
    self.set_skip_media_persist(false);
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
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let entity = crate::playback_entity::PlaybackEntity::resolve(&canonical);
    let db_key = entity.db_path();
    {
        let shell = self.me_budget_shell_path.borrow();
        let outgoing = media_probe::shell_media_path(&self.mpv, shell.as_deref());
        let outgoing_disp = outgoing
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "?".into());
        let same_entity = outgoing.as_ref().is_some_and(|p| {
            let out_ent = crate::playback_entity::PlaybackEntity::resolve(p).db_path();
            crate::video_ext::paths_same_file(&out_ent, &db_key)
        });
        if clear_outgoing_resume && !warm_preload {
            if let Some(p) = outgoing {
                media_probe::clear_resume_for_path(&p);
            }
        } else if snapshot_outgoing && !warm_preload && !same_entity {
            self.save_playback_state_for_close();
        }
        if entity.has_unified_timeline() {
            crate::dvd_vob_log::resume_open_log(format!(
                "load outgoing={outgoing_disp} same_entity={same_entity} snapshot={snapshot_outgoing} warm={warm_preload}"
            ));
        }
    }
    if entity.has_unified_timeline() {
        crate::dvd_entity::sanitize_stale_entity_playback(&canonical, 0.0);
    }
    let stored = resume_at
        .or_else(|| db::resume_pos(&db_key))
        .or_else(|| {
            entity
                .has_unified_timeline()
                .then(|| crate::dvd_ifo_parse::movie_entry_global_sec(&db_key))
                .flatten()
        });
    let (load_path, pending) = if let Some(global) = stored {
        let map = db::load_duration_map();
        match entity.resume_load_target(&canonical, global, &map) {
            Some((target, local)) => (target, Some(local)),
            None => {
                crate::dvd_vob_log::resume_open_log(format!(
                    "load resume_load_target failed global={global:.2} probe={}",
                    canonical.display()
                ));
                (canonical.clone(), None)
            }
        }
    } else {
        if entity.has_unified_timeline() {
            crate::dvd_vob_log::resume_open_log(format!(
                "load no stored resume entity={}",
                db_key.display()
            ));
        }
        (canonical.clone(), None)
    };
    if entity.has_unified_timeline() {
        crate::dvd_vob_log::resume_open_log(format!(
            "load global={stored:?} local={pending:?} file={} entity={}",
            load_path.display(),
            db_key.display()
        ));
    }
    let prev_shell = self.me_budget_shell_path.borrow().clone();
    if prev_shell
        .as_ref()
        .is_some_and(|p| crate::preview_debug::open_target_entity_changed(p, &load_path))
    {
        crate::seek_bar_preview::reset_on_main_media_change_from("load_file_path:entity_change");
    }
    let s = load_path.to_str().ok_or("media path is not valid UTF-8")?;
    self.warm_file_gen.set(self.warm_file_gen.get().wrapping_add(1));
    if entity.has_unified_timeline() {
        crate::dvd_vob_log::resume_open_log(format!(
            "load stashed pending={pending:?} hold={stored:?} gen={}",
            self.warm_file_gen.get()
        ));
    }
    self.clear_chapter_scrub_pause_hold();
    self.chapter_scrub_resume.set(false);
    self.dvd_chain_bar_sync.set(None);
    self.dvd_hold_global.set(if entity.has_unified_timeline() {
        stored
    } else {
        None
    });
    self.pending_resume.set(pending);
    self.set_me_budget_shell_path(&load_path);
    self.mpv
        .command("loadfile", &[s, "replace"])
        .map_err(|e| format!("{e:?}"))
}

}

include!("mpv_persistence_chapter_seek.rs");
