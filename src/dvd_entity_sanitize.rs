// Repair stale whole-disc entity rows and implausible DVD bar totals.

/// Max plausible single-title duration (whole-disc entity row).
pub(crate) const MAX_TITLE_ENTITY_SEC: f64 = 21_600.0;

fn entity_playback_plausible(duration: f64, resume: f64) -> bool {
    duration.is_finite()
        && duration > 0.0
        && duration <= MAX_TITLE_ENTITY_SEC
        && resume.is_finite()
        && resume >= 0.0
        && resume <= duration + 1.0
}

fn measured_title_total(chapter: &Path, live_local: f64) -> Option<f64> {
    let live_local = crate::dvd_vob_timeline::clamp_vob_duration(live_local);
    let map = crate::db::load_duration_map();
    let mut tl = crate::dvd_entity::build_title_timeline_with(
        chapter,
        &map,
        live_local,
        crate::dvd_entity::TimelineBuildOpts::CACHE_ONLY,
    )?;
    tl.scrub_implausible_durs();
    tl.infer_missing_from_siblings();
    let total = tl.total_sec;
    (total > 0.0).then_some(total)
}

/// Fix entity `duration_sec` / `time_pos_sec` when a legacy whole-disc row exceeds a title length.
pub(crate) fn sanitize_stale_entity_playback(chapter: &Path, live_local: f64) -> bool {
    let ent = crate::playback_entity::PlaybackEntity::resolve(chapter);
    let key = ent.db_path();
    let map = crate::db::load_duration_map();
    let Some(d) = map.get(key.to_str().unwrap_or("")).copied() else {
        return false;
    };
    let r = crate::db::resume_pos(&key).unwrap_or(0.0);
    if entity_playback_plausible(d, r) {
        return false;
    }
    let Some(total) = measured_title_total(chapter, live_local) else {
        if live_local <= 0.0 {
            crate::db::clear_duration(&key);
            eprintln!(
                "[rhino] load: dvd_entity_sanitize cleared stale duration old_d={d:.1} old_r={r:.1}"
            );
            return true;
        }
        let global = r.min(live_local);
        crate::db::set_playback(&key, live_local, global);
        eprintln!(
            "[rhino] load: dvd_entity_sanitize live_only old_d={d:.1} old_r={r:.1} -> total={live_local:.1} global={global:.1}"
        );
        return true;
    };
    let global = r.min(total);
    crate::db::set_playback(&key, total, global);
    eprintln!(
        "[rhino] load: dvd_entity_sanitize old_d={d:.1} old_r={r:.1} -> total={total:.1} global={global:.1}"
    );
    true
}

pub(crate) fn bar_total_plausible(total: f64, chapter_count: usize) -> bool {
    total.is_finite()
        && total > 0.0
        && total <= MAX_TITLE_ENTITY_SEC
        && total <= chapter_count as f64 * crate::dvd_vob_timeline::MAX_VOB_DUR_SEC
}

/// Drop cached headless probe hits for one title set before rebuilding the bar.
pub(crate) fn clear_title_probe_cache(chapter: &Path) {
    let Some(vobs) = crate::dvd_entity::timeline_chapter_paths(chapter) else {
        return;
    };
    crate::dvd_vob_mpv_probe::clear_probe_cache_for_paths(&vobs);
}
