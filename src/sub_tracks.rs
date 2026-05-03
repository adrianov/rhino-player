//! Subtitle stream list, popover rebuild, and Levenshtein auto-pick. See `docs/features/24-subtitles.md`.

use libmpv2::Mpv;
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::db::SubPrefs;
use crate::mpv_embed::MpvBundle;
use crate::sub_track_abbr::abbrev_track_lang;

type SubPickFn = Rc<dyn Fn(&str)>;
type SubOffFn = Rc<dyn Fn()>;

#[derive(Deserialize)]
struct Node {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

struct Row {
    id: i64,
    text: String,
    lang: String,
}

fn track_header_token(r: &Row) -> String {
    let l = r.lang.trim();
    if !l.is_empty() {
        let a = abbrev_track_lang(Some(l));
        if !a.is_empty() {
            return a;
        }
    }
    let head = r
        .text
        .split(" – ")
        .next()
        .unwrap_or(r.text.as_str())
        .trim();
    abbrev_track_lang(Some(head))
}

fn compact_header_label_row(sid: i64, rows: &[Row]) -> String {
    let Some(row) = rows.iter().find(|r| r.id == sid) else {
        return "…".to_string();
    };
    let t = track_header_token(row);
    if t.is_empty() {
        "…".into()
    } else {
        t
    }
}

/// Updates the subtitles header caption from the current subtitle track (`Off` when hidden).
pub fn refresh_sub_header(mpv: &Mpv, label: &gtk::Label) {
    let s = sub_header_compact(mpv);
    if label.text().as_str() != s.as_str() {
        label.set_text(&s);
    }
}

fn sub_header_compact(mpv: &Mpv) -> String {
    if !sub_visibility(mpv) {
        return "Off".to_string();
    }
    let sid = match current_sid(mpv) {
        Some(id) => id,
        None => return "Auto".to_string(),
    };
    let rows = sub_rows(mpv);
    compact_header_label_row(sid, &rows)
}

fn line_label(id: i64, title: Option<String>, lang: Option<String>) -> String {
    let t = title.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let l = lang.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let (Some(a), Some(b)) = (t, l) {
        return format!("{a} – {b}");
    }
    if let Some(s) = t.or(l) {
        return s.to_string();
    }
    format!("Track {id}")
}

fn sub_rows(mpv: &Mpv) -> Vec<Row> {
    let json = match mpv.get_property::<String>("track-list") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let nodes: Vec<Node> = serde_json::from_str(&json).unwrap_or_default();
    let mut v = vec![];
    for n in nodes {
        if n.kind != "sub" {
            continue;
        }
        let lang = n
            .lang
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("")
            .to_string();
        v.push(Row {
            id: n.id,
            text: line_label(n.id, n.title, n.lang),
            lang,
        });
    }
    v
}

/// `track-list` has at least one `type: sub` entry.
pub fn has_subtitle_tracks(mpv: &Mpv) -> bool {
    !sub_rows(mpv).is_empty() || has_subtitle_track_props(mpv)
}

fn has_subtitle_track_props(mpv: &Mpv) -> bool {
    let Ok(count) = mpv.get_property::<i64>("track-list/count") else {
        return false;
    };
    for i in 0..count.max(0) {
        let key = format!("track-list/{i}/type");
        if mpv.get_property::<String>(&key).is_ok_and(|s| s == "sub") {
            return true;
        }
    }
    false
}

/// Seeding string for Levenshtein: last hand-picked track label, else a short [LANG] hint.
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

/// After a new [loadfile], pick the subtitle track closest to [autoseed] (normalized Levenshtein).
pub fn autopick_sub_track(mpv: &Mpv, prefs: &SubPrefs) {
    if prefs.sub_off {
        set_sub_off(mpv);
        return;
    }
    let rows = sub_rows(mpv);
    if rows.is_empty() {
        return;
    }
    let seed = autoseed(prefs);
    if seed.is_empty() {
        return;
    }
    let mut best = 0.0_f64;
    let mut best_id: Option<i64> = None;
    for r in &rows {
        let text = r.text.to_lowercase();
        let lang = r.lang.to_lowercase();
        let sc = strsim::normalized_levenshtein(&seed, &text)
            .max(strsim::normalized_levenshtein(&seed, &lang));
        if sc > best {
            best = sc;
            best_id = Some(r.id);
        }
    }
    if best < 0.38 {
        return;
    }
    if let Some(id) = best_id {
        let _ = mpv.set_property("sub-visibility", true);
        let _ = mpv.set_property("sid", id);
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
        eprintln!("[rhino] set sid {id}");
    }
}

include!("sub_tracks_rebuild.rs");
