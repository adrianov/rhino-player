// Whole-title seek execution: stale-scrub abort, plan resolution, and the three seek paths
// (included from `dvd_vob_timeline_transport.rs`).

/// Bar-cached plan when available, unified-timeline rebuild otherwise.
fn resolve_seek_plan(
    b: &crate::mpv_embed::MpvBundle,
    dvd_bar: Option<&std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>>,
    shell: Option<&Path>,
    path: &Path,
    global_sec: f64,
) -> Option<SeekPlan> {
    dvd_bar
        .and_then(|slot| {
            let bar = slot.borrow();
            bar.as_ref()
                .and_then(|bar| seek_plan_from_bar(bar, path, global_sec))
        })
        .or_else(|| seek_plan_fallback(&b.mpv, shell, global_sec))
}

fn log_missing_seek_plan(path: &Path, global_sec: f64, bar_present: bool) {
    crate::dvd_vob_log::dvd_seek_log(format!(
        "seek_global: no plan global={global_sec:.2} chapter={} bar_cache={bar_present}",
        path.display()
    ));
}

/// Drop a stale in-flight chapter scrub before planning a fresh whole-title seek.
fn abort_stale_chapter_scrub(b: &mut crate::mpv_embed::MpvBundle) {
    if b.chapter_cross_load_busy() {
        b.apply_pending_resume();
        if b.chapter_cross_load_busy() {
            crate::dvd_vob_log::dvd_seek_log("seek_global: abort stale chapter scrub");
            b.abort_chapter_load(false);
        }
    }
}

fn seek_global_borrowed(
    g: &mut Option<crate::mpv_embed::MpvBundle>,
    global_sec: f64,
    dvd_bar: Option<&std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>>,
    resume_playing: bool,
) -> SeekGlobalOutcome {
    let Some(b) = g.as_mut() else {
        crate::dvd_vob_log::dvd_seek_log("seek_global: no player bundle");
        return SeekGlobalOutcome::UNHANDLED;
    };
    abort_stale_chapter_scrub(b);
    let shell = b.me_budget_shell_path.borrow().clone();
    let Some(path) = open_dvd_chapter_path(&b.mpv, shell.as_deref()) else {
        crate::dvd_vob_log::dvd_seek_log("seek_global: not a DVD chapter path");
        return SeekGlobalOutcome::UNHANDLED;
    };
    let bar_present = dvd_bar.is_some_and(|s| s.borrow().is_some());
    // `match` rather than let-else: the log path needs `bar_present`, and reads inside a
    // let-else else-block are not tracked by abcop's scope model (upstream bug).
    let plan = match resolve_seek_plan(b, dvd_bar, shell.as_deref(), &path, global_sec) {
        Some(p) => p,
        None => {
            log_missing_seek_plan(&path, global_sec, bar_present);
            return SeekGlobalOutcome::UNHANDLED;
        }
    };
    execute_seek_plan(b, plan, global_sec, resume_playing, dvd_bar)
}

fn execute_seek_plan(
    b: &mut crate::mpv_embed::MpvBundle,
    plan: SeekPlan,
    global_sec: f64,
    resume_playing: bool,
    dvd_bar: Option<&std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>>,
) -> SeekGlobalOutcome {
    let SeekPlan {
        current: path,
        target,
        local,
        g_target,
        from_bar,
    } = plan;
    let cross = !crate::video_ext::paths_same_file(target.as_path(), &path);
    crate::dvd_vob_log::dvd_seek_log(format!(
        "seek_global: global={global_sec:.2} -> g_target={g_target:.2} local={local:.2} cross_file={cross} bar={from_bar} target={}",
        target.display()
    ));
    let target = target.as_path();
    let chain_head = crate::dvd_vob_mpv_probe::is_title_chain_head(target);
    if cross || chain_head {
        if chain_head && !cross {
            seek_chain_head_ifo_local(b, target, local, g_target, dvd_bar);
            return SeekGlobalOutcome::HANDLED;
        }
        return seek_replace_media(b, target, local, g_target, resume_playing);
    }
    seek_in_place(b, g_target, local, dvd_bar);
    SeekGlobalOutcome::HANDLED
}

/// Chain-head chapter: hold the whole-title position and seek in IFO-local coordinates.
fn seek_chain_head_ifo_local(
    b: &mut crate::mpv_embed::MpvBundle,
    target: &Path,
    local: f64,
    g_target: f64,
    dvd_bar: Option<&std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>>,
) {
    b.dvd_hold_global.set(Some(g_target));
    crate::mpv_embed::seek_chain_ifo_local(&b.mpv, target, local);
    b.dvd_chain_bar_sync
        .set(Some(crate::dvd_vob_timeline::DvdChainBarSync::from_scrub(
            b, g_target, local,
        )));
    b.dvd_hold_global.set(None);
    persist_seek_global_entity(b, dvd_bar, g_target);
}

/// Cross-file seek: replace media with the target chapter at its local offset.
fn seek_replace_media(
    b: &mut crate::mpv_embed::MpvBundle,
    target: &Path,
    local: f64,
    g_target: f64,
    resume_playing: bool,
) -> SeekGlobalOutcome {
    crate::video_pref::strip_vapoursynth_before_replace_media(b);
    if b.load_chapter_seek(target, local, g_target, resume_playing, false)
        .is_err()
    {
        b.dvd_hold_global.set(None);
        b.clear_chapter_scrub_resume();
        crate::dvd_vob_log::dvd_seek_log("seek_global: load_chapter_seek failed");
        return SeekGlobalOutcome::UNHANDLED;
    }
    crate::app::transport_drain_after_loadfile_idle();
    SeekGlobalOutcome::HANDLED
}

/// Same-file seek: absolute-exact mpv seek under the held whole-title position.
fn seek_in_place(
    b: &mut crate::mpv_embed::MpvBundle,
    g_target: f64,
    local: f64,
    dvd_bar: Option<&std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>>,
) {
    b.dvd_hold_global.set(Some(g_target));
    b.dvd_chain_bar_sync.set(None);
    let s = format!("{local:.4}");
    let _ = crate::video_pref::unload_smooth_for_seek(&b.mpv, Some(b));
    let _ = b.mpv.command("seek", &[s.as_str(), "absolute+exact"]);
    persist_seek_global_entity(b, dvd_bar, g_target);
}
