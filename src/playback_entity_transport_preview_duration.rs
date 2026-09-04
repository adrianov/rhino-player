// Bar-hover duration caps: title-wide seconds for DVD entities, main/preview-file
// duration otherwise (included by `playback_entity_transport_preview.rs`).

impl PlaybackEntity {
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
            if let Some(cap) = unified_timeline_cap(chapter, main, bar_upper, dvd_bar) {
                return cap;
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

/// Title-wide cap from the cached bar, else rebuilt cache-only from the duration map.
fn unified_timeline_cap(
    chapter: &Path,
    main: &Mpv,
    bar_upper: f64,
    dvd_bar: Option<&DvdBarState>,
) -> Option<f64> {
    if let Some(bar) = dvd_bar {
        return Some(bar.total_sec().min(bar_upper).max(0.0));
    }
    crate::dvd_vob_timeline::DvdBarState::build_with_map_opts(
        chapter,
        main
            .get_property::<f64>("duration")
            .ok()
            .map(crate::dvd_vob_timeline::clamp_vob_duration)
            .unwrap_or(0.0),
        &crate::db::load_duration_map(),
        crate::dvd_entity::TimelineBuildOpts::CACHE_ONLY,
    )
    .map(|bar| bar.total_sec().min(bar_upper).max(0.0))
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
