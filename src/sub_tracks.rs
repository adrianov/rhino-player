//! Subtitle stream list, popover rebuild, and token-overlap auto-pick. See `docs/features/24-subtitles.md`.

use libmpv2::Mpv;
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::db::SubPrefs;
use crate::mpv_embed::MpvBundle;
use crate::sub_track_abbr::abbrev_track_lang;
use crate::track_label_match::{seed_row_score, subtitle_autopick_qualifies, LabelMatchScore};

type SubPickFn = Rc<dyn Fn(&str)>;
type SubOffFn = Rc<dyn Fn()>;

#[derive(Deserialize)]
struct Node {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    codec: Option<String>,
}

struct Row {
    id: i64,
    text: String,
    lang: String,
    ifo_slot: Option<u8>,
}

/// True for image-based subs where mpv ignores `sub-color` (VOBSUB, PGS, DVB, …).
pub fn is_bitmap_sub_codec(codec: &str) -> bool {
    let c = codec.trim();
    c.eq_ignore_ascii_case("dvd_sub")
        || c.eq_ignore_ascii_case("dvb_sub")
        || c.eq_ignore_ascii_case("dvbsub")
        || c.eq_ignore_ascii_case("dvb_teletext")
        || c.eq_ignore_ascii_case("teletext")
        || c.eq_ignore_ascii_case("pgs")
        || c.eq_ignore_ascii_case("pgssub")
        || c.eq_ignore_ascii_case("hdmv_pgs_subtitle")
        || c.eq_ignore_ascii_case("xsub")
}

/// Whether Rhino text styling (`sub-color`, `sub-scale`, border) applies to the active subtitle track.
#[must_use]
pub fn text_styling_applies(mpv: &Mpv) -> bool {
    text_styling_applies_codecs(mpv, &sub_stream_codecs(mpv))
}

/// Re-push saved subtitle appearance after `sid` changes (e.g. BDMV text track after PGS).
pub fn reapply_styling(mpv: &Mpv) {
    crate::sub_prefs::apply_mpv(mpv, &crate::db::load_sub());
}

fn text_styling_applies_codecs(mpv: &Mpv, subs: &[(i64, String)]) -> bool {
    if subs.is_empty() {
        return false;
    }
    if let Some(id) = current_sid(mpv) {
        return subs
            .iter()
            .find(|(tid, _)| *tid == id)
            .is_some_and(|(_, codec)| !is_bitmap_sub_codec(codec));
    }
    subs.iter()
        .any(|(_, codec)| !is_bitmap_sub_codec(codec))
}

pub fn sync_text_color_row(mpv: &Mpv, row: &impl IsA<gtk::Widget>) {
    sync_text_color_row_codecs(mpv, row, &sub_stream_codecs(mpv));
}

fn sync_text_color_row_codecs(mpv: &Mpv, row: &impl IsA<gtk::Widget>, codecs: &[(i64, String)]) {
    row.set_visible(text_styling_applies_codecs(mpv, codecs));
}

fn sub_stream_codecs(mpv: &Mpv) -> Vec<(i64, String)> {
    sub_nodes_from_track_list(mpv)
        .into_iter()
        .map(|n| (n.id, n.codec.unwrap_or_default()))
        .collect()
}

fn sub_nodes_from_track_list(mpv: &Mpv) -> Vec<Node> {
    let json = match mpv.get_property::<String>("track-list") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let nodes: Vec<Node> = serde_json::from_str(&json).unwrap_or_default();
    nodes.into_iter().filter(|n| n.kind == "sub").collect()
}

fn track_header_token(r: &Row) -> String {
    let l = r.lang.trim();
    if !l.is_empty() {
        let a = abbrev_track_lang(Some(l));
        if !a.is_empty() {
            return a;
        }
    }
    let head = r.text.split(" – ").next().unwrap_or(r.text.as_str()).trim();
    abbrev_track_lang(Some(head))
}

fn compact_header_label_row(sid: i64, rows: &[Row], mpv: &Mpv, shell: Option<&std::path::Path>) -> String {
    let Some(row) = row_for_sid(sid, rows, mpv, shell) else {
        return "…".to_string();
    };
    let t = track_header_token(row);
    if t.is_empty() {
        "…".into()
    } else {
        t
    }
}

fn row_for_sid<'a>(sid: i64, rows: &'a [Row], mpv: &Mpv, shell: Option<&std::path::Path>) -> Option<&'a Row> {
    rows.iter()
        .find(|r| r.id == sid)
        .or_else(|| {
            let slot = ifo_slot_for_sid(mpv, sid, shell)?;
            rows.iter().find(|r| r.ifo_slot == Some(slot))
        })
}

/// Updates the subtitles header caption from the current subtitle track (`Off` when hidden).
pub fn refresh_sub_header(mpv: &Mpv, label: &gtk::Label, shell: Option<&std::path::Path>) {
    let s = sub_header_compact(mpv, shell);
    if label.text().as_str() != s.as_str() {
        label.set_text(&s);
    }
}

fn sub_header_compact(mpv: &Mpv, shell: Option<&std::path::Path>) -> String {
    if !sub_visibility(mpv) {
        return "Off".to_string();
    }
    let rows = sub_rows(mpv, shell);
    if let Some(sid) = current_sid(mpv) {
        return compact_header_label_row(sid, &rows, mpv, shell);
    }
    let prefs = crate::db::load_sub();
    let saved = prefs.last_sub_label.trim();
    if !saved.is_empty() {
        for r in &rows {
            if r.text.eq_ignore_ascii_case(saved)
                || r.lang.eq_ignore_ascii_case(saved)
                || r.text.contains(saved)
            {
                return track_header_token(r);
            }
        }
        let a = abbrev_track_lang(Some(saved));
        if !a.is_empty() {
            return a;
        }
    }
    "Auto".to_string()
}

include!("sub_tracks_dvd.rs");
include!("sub_tracks_restore.rs");

/// Reapply saved styling + track after load; fuzzy auto-pick only when nothing is stored yet.
pub fn reapply_saved_or_autopick(mpv: &Mpv, prefs: &SubPrefs, shell: Option<&std::path::Path>) {
    if !restore_saved_sub(mpv, prefs, shell) {
        autopick_sub_track(mpv, prefs, shell);
    }
}

/// `track-list` has at least one `type: sub` entry (or title-set IFO subs on DVD).
pub fn has_subtitle_tracks(mpv: &Mpv, shell: Option<&std::path::Path>) -> bool {
    crate::playback_entity::entity_has_subtitles(mpv, shell)
}

/// Seeding text for fuzzy match: last hand-picked track label, else a short [LANG] hint.
pub fn autoseed(prefs: &SubPrefs) -> String {
    let t = prefs.last_sub_label.trim();
    if !t.is_empty() {
        return t.to_lowercase();
    }
    std::env::var("LANG")
        .ok()
        .and_then(|s| s.split('.').next().map(str::to_string))
        .unwrap_or_else(|| "en".into())
        .split('_')
        .next()
        .unwrap_or("en")
        .to_lowercase()
}

/// After a new [loadfile], pick the subtitle track whose label best matches [autoseed]
/// (word multiset overlap first, then alphanumeric character multiset overlap).
pub fn autopick_sub_track(mpv: &Mpv, prefs: &SubPrefs, shell: Option<&std::path::Path>) {
    if prefs.sub_off {
        set_sub_off(mpv);
        return;
    }
    let rows = sub_rows(mpv, shell);
    if rows.is_empty() {
        return;
    }
    let seed = autoseed(prefs);
    if seed.is_empty() {
        return;
    }
    let mut best_score = LabelMatchScore {
        word_intersection: 0,
        char_intersection: 0,
    };
    let mut best_id: Option<i64> = None;
    for r in &rows {
        let s = seed_row_score(&seed, &r.text, &r.lang);
        if best_id.is_none() || s > best_score {
            best_score = s;
            best_id = Some(r.id);
        }
    }
    if !subtitle_autopick_qualifies(best_score) {
        return;
    }
    if let Some(id) = best_id {
        let sid = rows
            .iter()
            .find(|r| r.id == id)
            .and_then(|row| resolve_sub_id(mpv, id, row.ifo_slot, shell))
            .unwrap_or(id);
        let _ = mpv.set_property("sub-visibility", true);
        let _ = mpv.set_property("sid", sid);
        reapply_styling(mpv);
    }
}

fn sub_visibility(mpv: &Mpv) -> bool {
    mpv.get_property::<bool>("sub-visibility").unwrap_or(true)
}

fn current_sid(mpv: &Mpv) -> Option<i64> {
    if !sub_visibility(mpv) {
        return None;
    }
    if let Ok(s) = mpv.get_property::<String>("sid") {
        if s == "no" || s == "auto" {
            return None;
        }
        if let Ok(n) = s.parse::<i64>() {
            return Some(n);
        }
    }
    match mpv.get_property::<i64>("sid") {
        Ok(n) if n >= 0 => Some(n),
        _ => None,
    }
}

fn set_sub_off(mpv: &Mpv) {
    let _ = mpv.set_property("sub-visibility", false);
}

fn set_sub_id(mpv: &Mpv, id: i64) {
    let _ = mpv.set_property("sub-visibility", true);
    if mpv.set_property("sid", id).is_err() {
        let _ = mpv.set_property("sid", id.to_string());
    }
}

include!("sub_tracks_rebuild.rs");

#[cfg(test)]
mod tests {
    use super::is_bitmap_sub_codec;

    #[test]
    fn bitmap_sub_codecs() {
        assert!(is_bitmap_sub_codec("dvd_sub"));
        assert!(is_bitmap_sub_codec("PGS"));
        assert!(is_bitmap_sub_codec("hdmv_pgs_subtitle"));
        assert!(!is_bitmap_sub_codec("ass"));
        assert!(!is_bitmap_sub_codec("subrip"));
        assert!(!is_bitmap_sub_codec("hdmv_text_subtitle"));
    }
}
