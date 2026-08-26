// Background completion of missing `.vob` segment lengths (included from `dvd_vob_bar.rs`).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

thread_local! {
    static TAIL_ACTIVE: AtomicBool = const { AtomicBool::new(false) };
}

pub(crate) fn schedule_dvd_bar_probe_tail(
    slot: Rc<RefCell<Option<DvdBarState>>>,
    chapter: PathBuf,
    live_dur: f64,
) {
    if TAIL_ACTIVE.with(|f| f.load(Ordering::Acquire)) {
        return;
    }
    set_tail_active(true);
    glib::idle_add_local(move || probe_tail_tick(&slot, &chapter, live_dur));
}

fn set_tail_active(active: bool) {
    TAIL_ACTIVE.with(|f| f.store(active, Ordering::Release));
}

/// One background idle tick: rebuild with probed lengths, stop when none are missing.
#[must_use]
fn probe_tail_tick(
    slot: &Rc<RefCell<Option<DvdBarState>>>,
    chapter: &Path,
    live_dur: f64,
) -> glib::ControlFlow {
    let map = probe_tail_map(slot, chapter);
    let old_total = current_total(slot);
    let bar = DvdBarState::build_with_map_opts(
        chapter,
        live_dur,
        &map,
        crate::dvd_entity::TimelineBuildOpts::BACKGROUND,
    );
    let missing = bar.as_ref().map(|b| b.tl.missing_dur_count()).unwrap_or(0);
    if probe_tail_implausible(&bar, chapter) {
        crate::dvd_entity::clear_title_probe_cache(chapter);
        set_tail_active(false);
        return glib::ControlFlow::Break;
    }
    *slot.borrow_mut() = bar;
    if missing > 0 {
        return glib::ControlFlow::Continue;
    }
    set_tail_active(false);
    log_probe_tail_done(slot, old_total);
    glib::ControlFlow::Break
}

/// Duration map for a tail rebuild: fresh DB values plus prior-bar entries unless IFO rules.
fn probe_tail_map(
    slot: &Rc<RefCell<Option<DvdBarState>>>,
    chapter: &Path,
) -> std::collections::HashMap<String, f64> {
    let mut map = crate::db::load_duration_map();
    if !ifo_timeline_authoritative(chapter) {
        if let Some(prior) = slot.borrow().as_ref() {
            merge_prior_durs(&mut map, prior);
        }
    }
    map
}

fn current_total(slot: &Rc<RefCell<Option<DvdBarState>>>) -> f64 {
    slot.borrow()
        .as_ref()
        .map(DvdBarState::total_sec)
        .unwrap_or(0.0)
}

/// True when a rebuilt bar's total is implausible against the on-disk segment count.
fn probe_tail_implausible(bar: &Option<DvdBarState>, chapter: &Path) -> bool {
    let on_disk_n = crate::dvd_entity::timeline_chapter_paths(chapter)
        .map(|c| c.len())
        .unwrap_or(0);
    implausible_total(bar, on_disk_n)
}

fn log_probe_tail_done(slot: &Rc<RefCell<Option<DvdBarState>>>, old_total: f64) {
    let guard = slot.borrow();
    let Some(new_b) = guard.as_ref() else {
        return;
    };
    let new_total = new_b.total_sec();
    if (new_total - old_total).abs() > 0.5 {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "dvd probe tail done: total={new_total:.1}s vobs={}",
            new_b.tl.vobs.len()
        ));
    }
}
