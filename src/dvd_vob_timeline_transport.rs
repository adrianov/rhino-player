// Transport bar, seek, and preview mapping for DVD unified timeline (included from `dvd_vob_timeline.rs`).

/// DVD title entity: map whole-title hover time → chapter `.vob` load + local seek.
pub(crate) struct DvdTitlePreviewPlan {
    pub load: String,
    pub local_sec: f64,
    pub chapter_dur: f64,
}

pub(crate) fn dvd_title_preview_plan(
    mpv: &libmpv2::Mpv,
    shell: Option<&Path>,
    global_t: f64,
    bar: Option<&DvdBarState>,
) -> Option<DvdTitlePreviewPlan> {
    let chapter = open_dvd_chapter_path(mpv, shell)?;
    let entity = crate::playback_entity::PlaybackEntity::resolve(&chapter);
    if !entity.has_unified_timeline() {
        return None;
    }
    let map = crate::db::load_duration_map();
    let active_bar = bar.filter(|b| entity.dvd_bar_active(&chapter, b));
    let open = open_dvd_chapter_path(mpv, shell)?;
    let mpv_dur = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    let open_cap = crate::dvd_entity::StillOpenCap {
        chapter: open,
        mpv_dur,
    };
    let still = entity.still_at_global(
        &chapter,
        global_t,
        &map,
        active_bar,
        Some(&open_cap),
    )?;
    crate::dvd_vob_log::dvd_seek_log(format!(
        "preview global={global_t:.2} -> {} local={:.2} ch_dur={:.2} (bar={})",
        still.load.display(),
        still.local_sec,
        still.chapter_dur,
        if active_bar.is_some() { "yes" } else { "no" }
    ));
    Some(DvdTitlePreviewPlan {
        load: still.load.to_str()?.to_string(),
        local_sec: still.local_sec,
        chapter_dur: still.chapter_dur,
    })
}

/// Seek the main player to a whole-title time (seconds). Returns `true` when handled.
pub fn seek_global(
    player: &std::rc::Rc<std::cell::RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    global_sec: f64,
    dvd_bar: Option<&std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>>,
    resume_playing: bool,
) -> bool {
    let outcome = match player.try_borrow_mut() {
        Ok(mut g) => seek_global_borrowed(&mut g, global_sec, dvd_bar, resume_playing),
        Err(_) => {
            let p = std::rc::Rc::clone(player);
            let bar = dvd_bar.map(std::rc::Rc::clone);
            let _ = glib::idle_add_local_once(move || {
                let _ = seek_global(&p, global_sec, bar.as_ref(), resume_playing);
            });
            return true;
        }
    };
    if outcome.drain_transport {
        crate::app::transport_drain_after_loadfile();
    }
    outcome.handled
}

struct SeekGlobalOutcome {
    handled: bool,
    drain_transport: bool,
}

struct SeekPlan {
    current: PathBuf,
    target: PathBuf,
    local: f64,
    g_target: f64,
    from_bar: bool,
}

fn bar_total_from_slot(dvd_bar: Option<&std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>>) -> f64 {
    dvd_bar
        .and_then(|s| s.borrow().as_ref().map(DvdBarState::total_sec))
        .filter(|t| t.is_finite() && *t > 0.0)
        .unwrap_or(0.0)
}

fn persist_seek_global_entity(
    b: &crate::mpv_embed::MpvBundle,
    dvd_bar: Option<&std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>>,
    global: f64,
) {
    let total = bar_total_from_slot(dvd_bar);
    if total > 0.0 {
        b.persist_entity_bar_global(total, global);
    }
}

fn seek_plan_from_bar(bar: &DvdBarState, chapter: &std::path::Path, global_sec: f64) -> Option<SeekPlan> {
    let total = bar.total_sec();
    if !(total > 0.0) {
        return None;
    }
    let g_target = global_sec.clamp(0.0, total);
    let (idx, local) = bar.resolve_global(g_target);
    let target = bar.path_at(idx)?.to_path_buf();
    Some(SeekPlan {
        current: chapter.to_path_buf(),
        target,
        local,
        g_target,
        from_bar: true,
    })
}

fn seek_plan_fallback(mpv: &libmpv2::Mpv, shell: Option<&std::path::Path>, global_sec: f64) -> Option<SeekPlan> {
    let path = open_dvd_chapter_path(mpv, shell)?;
    let local_dur = mpv
        .get_property::<f64>("duration")
        .ok()
        .map(crate::dvd_vob_timeline::clamp_vob_duration)
        .unwrap_or(0.0);
    let map = crate::db::load_duration_map();
    let tl = crate::dvd_entity::build_title_timeline(&path, &map, local_dur)?;
    let g_target = global_sec.clamp(0.0, tl.total_sec);
    let (idx, local) = tl.resolve_global(g_target);
    let target = tl.path_at(idx)?.to_path_buf();
    Some(SeekPlan {
        current: path,
        target,
        local,
        g_target,
        from_bar: false,
    })
}

fn seek_global_borrowed(
    g: &mut Option<crate::mpv_embed::MpvBundle>,
    global_sec: f64,
    dvd_bar: Option<&std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>>,
    resume_playing: bool,
) -> SeekGlobalOutcome {
    let Some(b) = g.as_mut() else {
        crate::dvd_vob_log::dvd_seek_log("seek_global: no player bundle");
        return SeekGlobalOutcome {
            handled: false,
            drain_transport: false,
        };
    };
    if b.chapter_cross_load_busy() {
        b.apply_pending_resume();
        if b.chapter_cross_load_busy() {
            crate::dvd_vob_log::dvd_seek_log("seek_global: abort stale chapter scrub");
            b.abort_chapter_load(false);
        }
    }
    let shell = b.me_budget_shell_path.borrow().clone();
    let Some(path) = open_dvd_chapter_path(&b.mpv, shell.as_deref()) else {
        crate::dvd_vob_log::dvd_seek_log("seek_global: not a DVD chapter path");
        return SeekGlobalOutcome {
            handled: false,
            drain_transport: false,
        };
    };
    let bar_present = dvd_bar.is_some_and(|s| s.borrow().is_some());
    let plan = dvd_bar
        .and_then(|slot| {
            let bar = slot.borrow();
            bar.as_ref()
                .and_then(|bar| seek_plan_from_bar(bar, &path, global_sec))
        })
        .or_else(|| seek_plan_fallback(&b.mpv, shell.as_deref(), global_sec));
    let Some(plan) = plan else {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "seek_global: no plan global={global_sec:.2} chapter={} bar_cache={bar_present}",
            path.display()
        ));
        return SeekGlobalOutcome {
            handled: false,
            drain_transport: false,
        };
    };
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
            b.dvd_hold_global.set(Some(g_target));
            crate::mpv_embed::seek_chain_ifo_local(&b.mpv, target, local);
            b.dvd_chain_bar_sync.set(Some(crate::dvd_vob_timeline::DvdChainBarSync::from_scrub(
                b, g_target, local,
            )));
            b.dvd_hold_global.set(None);
            persist_seek_global_entity(b, dvd_bar, g_target);
            return SeekGlobalOutcome {
                handled: true,
                drain_transport: false,
            };
        }
        crate::video_pref::strip_vapoursynth_before_replace_media(b);
        if b.load_chapter_seek(target, local, g_target, resume_playing, false).is_err() {
            b.dvd_hold_global.set(None);
            b.clear_chapter_scrub_resume();
            crate::dvd_vob_log::dvd_seek_log("seek_global: load_chapter_seek failed");
            return SeekGlobalOutcome {
                handled: false,
                drain_transport: false,
            };
        }
        crate::app::transport_drain_after_loadfile_idle();
        return SeekGlobalOutcome {
            handled: true,
            drain_transport: false,
        };
    }
    b.dvd_hold_global.set(Some(g_target));
    b.dvd_chain_bar_sync.set(None);
    let s = format!("{local:.4}");
    let _ = crate::video_pref::unload_smooth_on_pause(&b.mpv);
    let _ = b.mpv.command("seek", &[s.as_str(), "absolute+exact"]);
    persist_seek_global_entity(b, dvd_bar, g_target);
    SeekGlobalOutcome {
        handled: true,
        drain_transport: false,
    }
}
