// FileLoaded rebuild of the cached DVD bar (included from `dvd_vob_bar_refresh.rs`).

/// Rebuild cached bar state after `FileLoaded` / path change (not on every transport tick).
pub fn refresh_dvd_bar(
    slot: &std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>,
    mpv: &libmpv2::Mpv,
    shell: Option<&Path>,
) {
    let Some(chapter) = open_dvd_chapter_path(mpv, shell) else {
        *slot.borrow_mut() = None;
        return;
    };
    if !crate::playback_entity::PlaybackEntity::resolve(&chapter).uses_dvd_bar_cache() {
        *slot.borrow_mut() = None;
        return;
    }
    rebuild_and_store(slot, mpv, chapter);
}

/// Sanitize, rebuild and publish the bar for a confirmed DVD-bar entity.
fn rebuild_and_store(
    slot: &std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>,
    mpv: &libmpv2::Mpv,
    chapter: std::path::PathBuf,
) {
    let live = live_vob_duration(mpv);
    crate::dvd_entity::sanitize_stale_entity_playback(&chapter, live);
    let on_disk_n = crate::dvd_entity::timeline_chapter_paths(&chapter)
        .map(|c| c.len())
        .unwrap_or(0);
    let mut map = crate::db::load_duration_map();
    let ifo_bar = ifo_timeline_authoritative(&chapter);
    let prior_meta = merge_prior_meta(slot, &mut map, on_disk_n, ifo_bar);
    let bar = build_sanitized_bar(&chapter, live, on_disk_n, &map);
    if live == 0.0 && keep_prior_total(&bar, prior_meta) {
        return;
    }
    log_refresh_outcome(&bar, &chapter, on_disk_n);
    publish_bar(slot, bar, chapter, ifo_bar, live);
}

/// Store the rebuilt bar; arm background probing while segment lengths are missing.
fn publish_bar(
    slot: &std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>,
    bar: Option<DvdBarState>,
    chapter: std::path::PathBuf,
    ifo_bar: bool,
    live: f64,
) {
    let need_probe_tail = bar.as_ref().map_or(true, |b| b.tl.missing_dur_count() > 0) && !ifo_bar;
    *slot.borrow_mut() = bar;
    if need_probe_tail {
        schedule_dvd_bar_probe_tail(std::rc::Rc::clone(slot), chapter, live);
    }
}

/// Merge a still-plausible prior bar's per-file durations into the fresh map; returns its meta.
fn merge_prior_meta(
    slot: &std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>,
    map: &mut std::collections::HashMap<String, f64>,
    on_disk_n: usize,
    ifo_bar: bool,
) -> Option<(f64, usize)> {
    let guard = slot.borrow();
    let old = guard.as_ref()?;
    let meta = (old.total_sec(), old.tl.vobs.len());
    if crate::dvd_entity::bar_total_plausible(meta.0, on_disk_n) && !ifo_bar {
        merge_prior_durs(map, old);
    }
    Some(meta)
}

/// Build the bar, degrading to weaker duration sources until the total is plausible.
fn build_sanitized_bar(
    chapter: &Path,
    live: f64,
    on_disk_n: usize,
    map: &std::collections::HashMap<String, f64>,
) -> Option<DvdBarState> {
    let bar = DvdBarState::build_with_map(chapter, live, map);
    if !implausible_total(&bar, on_disk_n) {
        return bar;
    }
    crate::dvd_entity::clear_title_probe_cache(chapter);
    let bar = DvdBarState::build_with_map(chapter, live, &crate::db::load_duration_map());
    if !implausible_total(&bar, on_disk_n) {
        return bar;
    }
    log_live_only_fallback(&bar, on_disk_n);
    DvdBarState::build_with_map(chapter, live, &std::collections::HashMap::new())
}

/// True when a built bar's total is implausible against the on-disk segment count.
fn implausible_total(bar: &Option<DvdBarState>, on_disk_n: usize) -> bool {
    bar.as_ref()
        .is_some_and(|b| !crate::dvd_entity::bar_total_plausible(b.total_sec(), on_disk_n))
}

fn log_live_only_fallback(bar: &Option<DvdBarState>, on_disk_n: usize) {
    eprintln!(
        "[rhino] load: dvd_bar_sanitize rebuild live_only was={:.1}s vobs={on_disk_n}",
        bar.as_ref().map(DvdBarState::total_sec).unwrap_or(0.0)
    );
}

/// Keep the prior bar when a live-less rebuild inflated its total; logs the decision.
fn keep_prior_total(bar: &Option<DvdBarState>, prior_meta: Option<(f64, usize)>) -> bool {
    match inflated_vs_prior(bar, prior_meta) {
        Some((old_total, new_total)) => {
            crate::dvd_vob_log::dvd_seek_log(format!(
                "refresh_dvd_bar: keep prior total={old_total:.1}s (new={new_total:.1}s live=0)"
            ));
            true
        }
        None => false,
    }
}

/// A live-less rebuild whose total ballooned past the prior bar (`old_total`, new total).
fn inflated_vs_prior(
    bar: &Option<DvdBarState>,
    prior_meta: Option<(f64, usize)>,
) -> Option<(f64, f64)> {
    let new_b = bar.as_ref()?;
    let (old_total, old_n) = prior_meta?;
    (new_b.tl.vobs.len() == old_n && old_total > 60.0 && new_b.total_sec() > old_total * 1.5)
        .then(|| (old_total, new_b.total_sec()))
}

fn log_refresh_outcome(bar: &Option<DvdBarState>, chapter: &Path, on_disk_n: usize) {
    if let Some(b) = bar {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "refresh_dvd_bar: total={:.1}s vobs={} on_disk={on_disk_n} file={}",
            b.total_sec(),
            b.tl.vobs.len(),
            chapter.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        ));
    } else {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "refresh_dvd_bar: build failed for {}",
            chapter.display()
        ));
    }
}
