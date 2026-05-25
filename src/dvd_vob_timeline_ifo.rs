// IFO-authoritative DVD timeline: drop tiny menu stubs only.

use crate::dvd_ifo_parse::MIN_SUBSTANTIAL_SEC;

/// Whole-title PGC cell times are available for this chapter path.
pub(super) fn ifo_timeline_authoritative(chapter: &Path) -> bool {
    crate::dvd_ifo_parse::title_playback_sec(chapter).is_some()
}

/// Remove IFO-identified menu stubs (`VTS_xx_1` ≈1 s) from the unified queue.
pub(super) fn drop_ifo_stub_segments(tl: &mut DvdVobTimeline) {
    let mut vobs = Vec::with_capacity(tl.vobs.len());
    let mut durs = Vec::with_capacity(tl.durs.len());
    for (vob, &d) in tl.vobs.iter().zip(tl.durs.iter()) {
        if d > 0.0 && d < MIN_SUBSTANTIAL_SEC && !crate::dvd_entity::chapter_vob_substantial_on_disk(vob)
        {
            continue;
        }
        vobs.push(vob.clone());
        durs.push(d);
    }
    if vobs.len() == tl.vobs.len() || vobs.is_empty() {
        return;
    }
    tl.vobs = vobs;
    tl.durs = durs;
    tl.recompute_starts();
}
