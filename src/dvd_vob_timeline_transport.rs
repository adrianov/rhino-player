// Transport bar, seek, and preview mapping for DVD unified timeline (included from `dvd_vob_timeline.rs`).

include!("dvd_vob_transport_seek.rs");

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
    let still = entity.still_at_global(
        &chapter,
        global_t,
        &map,
        active_bar,
        Some(&open_still_cap(mpv, &chapter)),
    )?;
    log_title_preview(global_t, &still, active_bar.is_some());
    preview_plan_from(still)
}

/// Live-open cap describing the chapter mpv is currently decoding (pure query).
fn open_still_cap(mpv: &libmpv2::Mpv, chapter: &Path) -> crate::dvd_entity::StillOpenCap {
    let mpv_dur = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    crate::dvd_entity::StillOpenCap {
        chapter: chapter.to_path_buf(),
        mpv_dur,
    }
}

fn log_title_preview(global_t: f64, still: &crate::dvd_entity::DvdStillTarget, bar_active: bool) {
    crate::dvd_vob_log::dvd_seek_log(format!(
        "preview global={global_t:.2} -> {} local={:.2} ch_dur={:.2} (bar={})",
        still.load.display(),
        still.local_sec,
        still.chapter_dur,
        if bar_active { "yes" } else { "no" }
    ));
}

fn preview_plan_from(still: crate::dvd_entity::DvdStillTarget) -> Option<DvdTitlePreviewPlan> {
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

impl SeekGlobalOutcome {
    const HANDLED: Self = Self {
        handled: true,
        drain_transport: false,
    };

    const UNHANDLED: Self = Self {
        handled: false,
        drain_transport: false,
    };
}

struct SeekPlan {
    current: PathBuf,
    target: PathBuf,
    local: f64,
    g_target: f64,
    from_bar: bool,
}

fn bar_total_from_slot(
    dvd_bar: Option<&std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>>,
) -> f64 {
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

fn seek_plan_from_bar(
    bar: &DvdBarState,
    chapter: &std::path::Path,
    global_sec: f64,
) -> Option<SeekPlan> {
    let total = bar.total_sec();
    if total.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
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

fn seek_plan_fallback(
    mpv: &libmpv2::Mpv,
    shell: Option<&std::path::Path>,
    global_sec: f64,
) -> Option<SeekPlan> {
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
