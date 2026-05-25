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
    TAIL_ACTIVE.with(|f| f.store(true, Ordering::Release));
    glib::idle_add_local(move || {
        let mut map = crate::db::load_duration_map();
        if !ifo_timeline_authoritative(&chapter) {
            if let Some(prior) = slot.borrow().as_ref() {
                merge_prior_durs(&mut map, prior);
            }
        }
        let old_total = slot.borrow().as_ref().map(DvdBarState::total_sec).unwrap_or(0.0);
        let bar = DvdBarState::build_with_map_opts(
            &chapter,
            live_dur,
            &map,
            crate::dvd_entity::TimelineBuildOpts::BACKGROUND,
        );
        let missing = bar.as_ref().map(|b| b.tl.missing_dur_count()).unwrap_or(0);
        let on_disk_n = crate::dvd_entity::timeline_chapter_paths(&chapter)
            .map(|c| c.len())
            .unwrap_or(0);
        if bar.as_ref().is_some_and(|b| {
            !crate::dvd_entity::bar_total_plausible(b.total_sec(), on_disk_n)
        }) {
            crate::dvd_entity::clear_title_probe_cache(&chapter);
            TAIL_ACTIVE.with(|f| f.store(false, Ordering::Release));
            return glib::ControlFlow::Break;
        }
        *slot.borrow_mut() = bar;
        if missing > 0 {
            return glib::ControlFlow::Continue;
        }
        TAIL_ACTIVE.with(|f| f.store(false, Ordering::Release));
        if let Some(new_b) = slot.borrow().as_ref() {
            let new_total = new_b.total_sec();
            if (new_total - old_total).abs() > 0.5 {
                crate::dvd_vob_log::dvd_seek_log(format!(
                    "dvd probe tail done: total={new_total:.1}s vobs={}",
                    new_b.tl.vobs.len()
                ));
            }
        }
        glib::ControlFlow::Break
    });
}
