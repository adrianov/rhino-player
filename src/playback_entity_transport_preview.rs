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
        shell.and_then(|p| {
            std::fs::canonicalize(p)
                .ok()
                .or_else(|| Some(p.to_path_buf()))
        })
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
    ent.preview_seek_plan(PreviewSeekCtx {
        chapter: &chapter,
        mpv,
        shell,
        hover_global,
        bar_upper,
        dvd_bar: active_bar,
        preview_mpv,
    })
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
    Some(ent.preview_hover_duration(&chapter, bar_upper, mpv, preview_mpv, active_bar))
}

/// Inputs for one hover-preview plan on the open entity.
struct PreviewSeekCtx<'a> {
    chapter: &'a Path,
    mpv: &'a Mpv,
    shell: Option<&'a Path>,
    hover_global: f64,
    bar_upper: f64,
    dvd_bar: Option<&'a DvdBarState>,
    preview_mpv: Option<&'a Mpv>,
}

impl PlaybackEntity {
    /// True when a cached [DvdBarState] may apply to this entity.
    #[must_use]
    pub fn uses_dvd_bar_cache(&self) -> bool {
        self.has_unified_timeline()
    }

    fn preview_seek_plan(&self, ctx: PreviewSeekCtx<'_>) -> Option<PreviewSeekPlan> {
        let PreviewSeekCtx {
            chapter,
            mpv,
            shell,
            hover_global,
            bar_upper,
            dvd_bar,
            preview_mpv,
        } = ctx;
        match &self.kind {
            super::PlaybackEntityKind::SingleFile(_) => {
                let load = single_file_preview_load(mpv, shell, chapter)?;
                let content_dur =
                    self.preview_hover_duration(chapter, bar_upper, mpv, preview_mpv, dvd_bar);
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
}

include!("playback_entity_transport_preview_duration.rs");

/// `loadfile` target from the mpv `path` property, when it is a usable local/stream path.
fn mpv_path_load(mpv: &Mpv) -> Option<String> {
    let t = mpv.get_property::<String>("path").ok()?.trim().to_string();
    if t.starts_with("bd://") || t.starts_with("bluray://") {
        return Some(t);
    }
    let p = crate::media_probe::local_path_from_mpv_str(&t)?;
    if p.is_file() && crate::video_ext::is_openable_media_path(&p) {
        return p.to_str().map(str::to_string);
    }
    None
}

fn single_file_preview_load(mpv: &Mpv, shell: Option<&Path>, chapter: &Path) -> Option<String> {
    if let Some(load) = mpv_path_load(mpv) {
        return Some(load);
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
