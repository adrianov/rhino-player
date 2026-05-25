// Seek-bar preview routing: single-file vs DVD title entity (included from transport).

use std::cell::RefCell;

/// Auxiliary-player `loadfile` target and seek for one transport-bar hover time.
pub struct PreviewSeekPlan {
    pub load: String,
    pub local_sec: f64,
    pub content_dur: f64,
}

/// Classify open playback: entity + local chapter / file path from mpv or shell.
#[must_use]
pub fn open_playback(mpv: &Mpv, shell: Option<&Path>) -> Option<(PlaybackEntity, PathBuf)> {
    let chapter = crate::media_probe::local_file_from_mpv(mpv).or_else(|| {
        shell.and_then(|p| std::fs::canonicalize(p).ok().or_else(|| Some(p.to_path_buf())))
    })?;
    Some((PlaybackEntity::resolve(&chapter), chapter))
}

/// Hover preview plan for whatever entity is open (DVD title or single file).
#[must_use]
pub fn preview_seek_plan_for_open(
    mpv: &Mpv,
    shell: Option<&Path>,
    hover_global: f64,
    bar_upper: f64,
    dvd_bar: Option<&RefCell<Option<DvdBarState>>>,
    preview_mpv: Option<&Mpv>,
) -> Option<PreviewSeekPlan> {
    let (ent, chapter) = open_playback(mpv, shell)?;
    let bar_hold = dvd_bar.map(|slot| slot.borrow());
    let active_bar = bar_hold
        .as_ref()
        .and_then(|g| g.as_ref())
        .filter(|b| ent.dvd_bar_active(&chapter, b));
    ent.preview_seek_plan(
        &chapter,
        mpv,
        shell,
        hover_global,
        bar_upper,
        active_bar,
        preview_mpv,
    )
}

/// Cap duration for preview hover / label on the open entity.
#[must_use]
pub fn preview_hover_duration_for_open(
    mpv: &Mpv,
    shell: Option<&Path>,
    bar_upper: f64,
    preview_mpv: Option<&Mpv>,
    dvd_bar: Option<&RefCell<Option<DvdBarState>>>,
) -> Option<f64> {
    let (ent, chapter) = open_playback(mpv, shell)?;
    let bar_hold = dvd_bar.map(|slot| slot.borrow());
    let active_bar = bar_hold
        .as_ref()
        .and_then(|g| g.as_ref())
        .filter(|b| ent.dvd_bar_active(&chapter, b));
    Some(ent.preview_hover_duration(
        &chapter,
        bar_upper,
        mpv,
        preview_mpv,
        active_bar,
    ))
}

impl PlaybackEntity {
    /// True when a cached [DvdBarState] may apply to this entity.
    #[must_use]
    pub fn uses_dvd_bar_cache(&self) -> bool {
        self.has_unified_timeline()
    }

    fn preview_seek_plan(
        &self,
        chapter: &Path,
        mpv: &Mpv,
        shell: Option<&Path>,
        hover_global: f64,
        bar_upper: f64,
        dvd_bar: Option<&DvdBarState>,
        preview_mpv: Option<&Mpv>,
    ) -> Option<PreviewSeekPlan> {
        match &self.kind {
            super::PlaybackEntityKind::SingleFile(_) => {
                let load = single_file_preview_load(mpv, shell, chapter)?;
                let content_dur = self.preview_hover_duration(
                    chapter,
                    bar_upper,
                    mpv,
                    preview_mpv,
                    dvd_bar,
                );
                Some(PreviewSeekPlan {
                    load,
                    local_sec: hover_global,
                    content_dur,
                })
            }
            super::PlaybackEntityKind::DvdTitle { .. } => {
                let plan = crate::dvd_vob_timeline::dvd_title_preview_plan(
                    mpv,
                    shell,
                    hover_global,
                    dvd_bar,
                )?;
                let content_dur = if plan.chapter_dur > 0.0 {
                    plan.chapter_dur
                } else {
                    bar_upper
                };
                Some(PreviewSeekPlan {
                    load: plan.load,
                    local_sec: plan.local_sec,
                    content_dur,
                })
            }
        }
    }

    /// Bar hover cap: title-wide for DVD entities; main-file duration for single files.
    #[must_use]
    pub fn preview_hover_duration(
        &self,
        chapter: &Path,
        bar_upper: f64,
        main: &Mpv,
        preview_mpv: Option<&Mpv>,
        dvd_bar: Option<&DvdBarState>,
    ) -> f64 {
        if self.has_unified_timeline() {
            if let Some(bar) = dvd_bar {
                return bar.total_sec().min(bar_upper).max(0.0);
            }
            let live = main
                .get_property::<f64>("duration")
                .ok()
                .map(crate::dvd_vob_timeline::clamp_vob_duration)
                .unwrap_or(0.0);
            let map = crate::db::load_duration_map();
            if let Some(bar) = crate::dvd_vob_timeline::DvdBarState::build_with_map_opts(
                chapter,
                live,
                &map,
                crate::dvd_entity::TimelineBuildOpts::CACHE_ONLY,
            ) {
                return bar.total_sec().min(bar_upper).max(0.0);
            }
        }
        let mut dur = bar_upper;
        if let Ok(d) = main.get_property::<f64>("duration") {
            if d.is_finite() && d > 0.0 {
                dur = dur.min(d);
            }
        }
        dur = min_preview_mpv_duration(self, main, preview_mpv, dur);
        dur.max(0.0)
    }
}

fn preview_mpv_duration_applies(ent: &PlaybackEntity, main: &Mpv, preview: &Mpv) -> bool {
    if ent.has_unified_timeline() {
        return true;
    }
    preview_mpv_matches_main(main, preview)
}

fn min_preview_mpv_duration(
    ent: &PlaybackEntity,
    main: &Mpv,
    preview_mpv: Option<&Mpv>,
    dur: f64,
) -> f64 {
    let Some(pr) = preview_mpv else {
        return dur;
    };
    if !preview_mpv_duration_applies(ent, main, pr) {
        return dur;
    }
    let Ok(d) = pr.get_property::<f64>("duration") else {
        return dur;
    };
    if d.is_finite() && d > 0.0 {
        dur.min(d)
    } else {
        dur
    }
}

fn preview_mpv_matches_main(main: &Mpv, preview: &Mpv) -> bool {
    match (
        crate::media_probe::local_file_from_mpv(main),
        crate::media_probe::local_file_from_mpv(preview),
    ) {
        (Some(a), Some(b)) => crate::video_ext::paths_same_file(&a, &b),
        _ => false,
    }
}

fn single_file_preview_load(mpv: &Mpv, shell: Option<&Path>, chapter: &Path) -> Option<String> {
    if let Ok(s) = mpv.get_property::<String>("path") {
        let t = s.trim();
        if t.starts_with("bd://") || t.starts_with("bluray://") {
            return Some(t.to_string());
        }
        if let Some(p) = crate::media_probe::local_path_from_mpv_str(t) {
            if p.is_file() && crate::video_ext::is_openable_media_path(&p) {
                return p.to_str().map(str::to_string);
            }
        }
    }
    if let Some(shell_p) = shell.filter(|p| p.exists()) {
        let resolved = crate::video_ext::resolve_open_media_path(shell_p);
        if crate::video_ext::is_optical_disc_path(&resolved) {
            return resolved.to_str().map(str::to_string);
        }
    }
    let resolved = crate::video_ext::resolve_open_media_path(chapter);
    resolved.to_str().map(str::to_string)
}
