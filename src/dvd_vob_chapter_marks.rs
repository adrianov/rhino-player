// IFO PTT chapter labels scaled onto the VOB unified timeline (display only).

use crate::dvd_ifo_parse::{chapter_marks_from_vob, IfoChapterMarks};

/// Map IFO PTT marks onto VOB-timeline seconds for seek-bar / preview labels.
pub(super) fn chapter_labels_from_ifo(chapter: &std::path::Path, vob_total: f64) -> Vec<(f64, String)> {
    chapter_marks_from_vob(chapter).map_or_else(Vec::new, |ifo| scale_ifo_labels(&ifo, vob_total))
}

fn scale_ifo_labels(ifo: &IfoChapterMarks, vob_total: f64) -> Vec<(f64, String)> {
    if ifo.mark_secs.is_empty() {
        return Vec::new();
    }
    let cap = vob_total.max(0.0);
    let scale = if ifo.title_sec > 0.0 && cap > 0.0 {
        cap / ifo.title_sec
    } else {
        1.0
    };
    let mut out = vec![(0.0, "Chapter 1".to_string())];
    for (i, &m) in ifo.mark_secs.iter().enumerate() {
        let t = (m * scale).clamp(0.0, cap);
        out.push((t, format!("Chapter {}", i + 2)));
    }
    out
}
