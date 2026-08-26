// DVD bar rebuild triggers and sanitizing reconstruction (included from `dvd_vob_bar.rs`).

/// Before `.vob` EOF advance: rebuild when the bar still looks like a single-file title.
pub fn refresh_dvd_bar_at_chapter_eof(
    slot: &std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>,
    mpv: &libmpv2::Mpv,
    shell: Option<&Path>,
) {
    let Some(chapter) = open_dvd_chapter_path(mpv, shell) else {
        return;
    };
    if !chapter_eof_pending(mpv, slot, &chapter) {
        return;
    }
    if !multi_segment_title(&chapter) {
        return;
    }
    if !stale_against_disk(slot, mpv, &chapter) {
        return;
    }
    refresh_dvd_bar(slot, mpv, shell);
}

/// Open chapter sits within the EOF tail window under its cached timeline.
fn chapter_eof_pending(
    mpv: &libmpv2::Mpv,
    slot: &std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>,
    chapter: &Path,
) -> bool {
    let guard = slot.borrow();
    let tl = guard.as_ref().map(|b| &b.tl);
    chapter_local_at_eof_for(mpv, Some(chapter), tl)
}

/// More than one `.vob` part exists on disk for the open title.
fn multi_segment_title(chapter: &Path) -> bool {
    crate::dvd_entity::timeline_chapter_paths(chapter).is_some_and(|c| c.len() > 1)
}

/// Cached bar disagrees with the on-disk title structure around the open chapter.
fn stale_against_disk(
    slot: &std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>,
    mpv: &libmpv2::Mpv,
    chapter: &Path,
) -> bool {
    let on_disk_n = crate::dvd_entity::timeline_chapter_paths(chapter)
        .map(|c| c.len())
        .unwrap_or(0);
    slot.borrow()
        .as_ref()
        .map_or(true, |b| chapter_eof_bar_stale(b, mpv, chapter, on_disk_n))
}

/// Stale when the cached bar misses segments, lacks a next chapter, or disagrees with mpv.
fn chapter_eof_bar_stale(
    b: &DvdBarState,
    mpv: &libmpv2::Mpv,
    chapter: &Path,
    on_disk_n: usize,
) -> bool {
    b.tl.vobs.len() < on_disk_n
        || b.tl.next_chapter_after(chapter).is_none()
        || capped_at_live_length(b, mpv)
        || shorter_than_live_segment(b, mpv, chapter)
}

/// Cached total is still just the single open file's mpv length.
fn capped_at_live_length(b: &DvdBarState, mpv: &libmpv2::Mpv) -> bool {
    b.mpv_chapter_duration(mpv)
        .is_some_and(|live| live > 0.0 && b.total_sec() <= live * 1.05)
}

/// The open segment's cached length trails what mpv actually reports for it.
fn shorter_than_live_segment(b: &DvdBarState, mpv: &libmpv2::Mpv, chapter: &Path) -> bool {
    b.mpv_chapter_duration(mpv).is_some_and(|live| {
        b.tl.index_of(chapter)
            .is_some_and(|i| live + 0.5 < b.tl.chapter_dur_at(i))
    })
}

include!("dvd_vob_bar_rebuild.rs");
