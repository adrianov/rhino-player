// IFO PTT chapter labels scaled onto the VOB unified timeline (display only).

use crate::dvd_ifo_parse::{chapter_marks_from_vob, IfoChapterMarks};

/// Build seek-bar / preview labels from every `VTS_xx_0.IFO` in the feature queue.
pub(super) fn chapter_labels_for_timeline(tl: &DvdVobTimeline) -> Vec<(f64, String)> {
    if tl.vobs.len() <= 1 {
        return Vec::new();
    }
    let mut out = vec![(0.0, "Chapter 1".to_string())];
    let mut chapter_n = 1;
    let mut i = 0;
    while i < tl.vobs.len() {
        let tid = crate::dvd_entity::vob_title_id(&tl.vobs[i]);
        let mut j = i + 1;
        while j < tl.vobs.len() {
            if crate::dvd_entity::vob_title_id(&tl.vobs[j]) != tid {
                break;
            }
            j += 1;
        }
        let group_start = tl.global_pos(&tl.vobs[i], 0.0);
        if i > 0 {
            chapter_n += 1;
            out.push((group_start, format!("Chapter {chapter_n}")));
        }
        let group_vob_total: f64 = (i..j).map(|k| tl.chapter_dur_at(k)).sum();
        if let Some(ifo) = chapter_marks_from_vob(&tl.vobs[i]) {
            append_scaled_ifo_marks(
                &mut out,
                &ifo,
                group_start,
                group_vob_total,
                tl.total_sec,
                &mut chapter_n,
            );
        } else {
            append_vob_part_marks(&mut out, tl, i, j, &mut chapter_n);
        }
        i = j;
    }
    if out.len() <= 1 {
        Vec::new()
    } else {
        out
    }
}

fn append_scaled_ifo_marks(
    out: &mut Vec<(f64, String)>,
    ifo: &IfoChapterMarks,
    group_start: f64,
    group_vob_total: f64,
    total_sec: f64,
    chapter_n: &mut u32,
) {
    if ifo.mark_secs.is_empty() || group_vob_total <= 0.0 || ifo.title_sec <= 0.0 {
        return;
    }
    let scale = group_vob_total / ifo.title_sec;
    for &m in &ifo.mark_secs {
        let t = (group_start + m * scale).clamp(0.0, total_sec);
        if out.iter().any(|(prev, _)| (prev - t).abs() < 0.05) {
            continue;
        }
        *chapter_n += 1;
        out.push((t, format!("Chapter {chapter_n}")));
    }
}

fn append_vob_part_marks(
    out: &mut Vec<(f64, String)>,
    tl: &DvdVobTimeline,
    from: usize,
    to: usize,
    chapter_n: &mut u32,
) {
    for k in (from + 1)..to {
        let t = tl.global_pos(&tl.vobs[k], 0.0);
        if out.iter().any(|(prev, _)| (prev - t).abs() < 0.05) {
            continue;
        }
        *chapter_n += 1;
        out.push((t, format!("Chapter {chapter_n}")));
    }
}

/// Only shrink preview seek range when the hover is near the next IFO mark (same-file chapter boundary).
const PREVIEW_MARK_NEAR_SEC: f64 = 30.0;

/// Preview seek cap: min open-`.vob` length and time until the next IFO chapter mark.
pub(crate) fn preview_chapter_dur(
    bar: &DvdBarState,
    global_t: f64,
    idx: usize,
    local: f64,
    load: &Path,
    map: &std::collections::HashMap<String, f64>,
) -> f64 {
    let mut dur = bar.chapter_dur_at(idx);
    if dur <= 0.0 {
        dur = dur_from_map(map, load);
    }
    if dur <= 0.0 {
        return local + 1.0;
    }
    if let Some(&(next_t, _)) = bar.chapter_labels.iter().find(|(t, _)| *t > global_t + 0.05)
    {
        let remain = next_t - global_t;
        if remain > 0.0 && remain <= PREVIEW_MARK_NEAR_SEC {
            dur = dur.min(local + remain);
        }
    }
    dur.max(0.0)
}
