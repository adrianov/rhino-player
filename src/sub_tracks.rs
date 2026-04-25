//! Subtitle stream list, popover rebuild, and Levenshtein auto-pick. See `docs/features/24-subtitles.md`.

use libmpv2::Mpv;
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::db::SubPrefs;
use crate::mpv_embed::MpvBundle;

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
    !sub_rows(mpv).is_empty()
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

/// Rebuild radio rows: **Off** + each sub. Returns **true** if any sub track exists.
///
/// [on_pick] is called with the list label when the user turns **on** a sub track (not **Off**).
/// [on_sub_off] when the user selects **Off** (persist so new files skip Levenshtein and stay off).
pub fn rebuild_popover(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    block: &Rc<Cell<bool>>,
    gl: &gtk::GLArea,
    on_pick: Option<SubPickFn>,
    on_sub_off: Option<SubOffFn>,
) -> bool {
    while let Some(c) = bx.first_child() {
        bx.remove(&c);
    }
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    let mpv = &b.mpv;
    let rows = sub_rows(mpv);
    if rows.is_empty() {
        return false;
    }
    let off_active = !sub_visibility(mpv);
    let want = current_sid(mpv);
    let fallback = rows.first().map(|r| r.id);
    block.set(true);
    let p = Rc::clone(player);
    let gl2 = gl.clone();
    let mut items: Vec<(i64, gtk::CheckButton)> = vec![];

    let off_btn = gtk::CheckButton::with_label("Off");
    let first = off_btn.clone();
    let p_off = Rc::clone(&p);
    let bl_off = Rc::clone(block);
    let g_off = gl2.clone();
    let off_cb = on_sub_off.as_ref().map(Rc::clone);
    off_btn.connect_toggled(move |b| {
        if bl_off.get() || !b.is_active() {
            return;
        }
        if let Some(pl) = p_off.borrow().as_ref() {
            set_sub_off(&pl.mpv);
        }
        if let Some(f) = off_cb.as_ref() {
            f();
        }
        g_off.queue_render();
    });
    items.push((-1, off_btn));

    for r in &rows {
        let btn = gtk::CheckButton::with_label(&r.text);
        btn.set_group(Some(&first));
        let id = r.id;
        let label = r.text.clone();
        let p2 = Rc::clone(&p);
        let blk2 = Rc::clone(block);
        let gl3 = gl2.clone();
        let pick = on_pick.as_ref().map(Rc::clone);
        btn.connect_toggled(move |b| {
            if blk2.get() || !b.is_active() {
                return;
            }
            if let Some(pl) = p2.borrow().as_ref() {
                set_sub_id(&pl.mpv, id);
            }
            if let Some(f) = pick.as_ref() {
                f(&label);
            }
            gl3.queue_render();
        });
        items.push((r.id, btn));
    }

    for (_, btn) in &items {
        bx.append(btn);
    }
    for (id, btn) in &items {
        if *id == -1 {
            btn.set_active(off_active);
        } else {
            let on = if off_active {
                false
            } else {
                want.or(fallback) == Some(*id)
            };
            btn.set_active(on);
        }
    }
    block.set(false);
    true
}
