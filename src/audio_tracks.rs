//! Audio stream list and `aid` for the sound popover. See `docs/features/08-tracks.md`.

use crate::mpv_embed::MpvBundle;
use libmpv2::Mpv;
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

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

fn audio_rows(mpv: &Mpv) -> Vec<Row> {
    let json = match mpv.get_property::<String>("track-list") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let nodes: Vec<Node> = serde_json::from_str(&json).unwrap_or_default();
    let mut v = vec![];
    for n in nodes {
        if n.kind != "audio" {
            continue;
        }
        v.push(Row {
            id: n.id,
            text: line_label(n.id, n.title, n.lang),
        });
    }
    v
}

fn current_aid(mpv: &Mpv) -> Option<i64> {
    if let Ok(s) = mpv.get_property::<String>("aid") {
        if s == "no" {
            return None;
        }
        if let Ok(n) = s.parse::<i64>() {
            return Some(n);
        }
    }
    match mpv.get_property::<i64>("aid") {
        Ok(n) if n > 0 => Some(n),
        _ => None,
    }
}

fn set_aid(mpv: &Mpv, id: i64) {
    if mpv.set_property("aid", id).is_err() {
        eprintln!("[rhino] set aid {id}");
    }
}

/// After [loadfile], make sure an audio stream is actually selected. With **one** [audio] track
/// there is no popover to pick it, and some files leave [aid] as `no` or unresolved until
/// [aid] is set explicitly. With **several** tracks, only fix explicit `aid=no` (e.g. stale state).
///
/// Do **not** set `aid` when that track is already active: repeating `set_property` on the same id
/// can re-open the audio path and leave A/V slightly out of sync (noticeable on Next / Previous,
/// where the delayed subtitle/audio hook runs after every `loadfile`).
pub fn ensure_playable_audio(mpv: &Mpv) {
    let rows = audio_rows(mpv);
    if rows.is_empty() {
        return;
    }
    if rows.len() == 1 {
        let want = rows[0].id;
        if current_aid(mpv) == Some(want) {
            return;
        }
        set_aid(mpv, want);
        return;
    }
    if let Ok(s) = mpv.get_property::<String>("aid") {
        if s == "no" {
            set_aid(mpv, rows[0].id);
        }
    }
}

/// Rebuilds radio rows. Returns **true** if there are **at least two** audio tracks (the block is a choice, not a duplicate for one track). Clears the box if there is no player or 0–1 track.
pub fn rebuild_popover(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    block: &Rc<Cell<bool>>,
    gl: &gtk::GLArea,
) -> bool {
    while let Some(c) = bx.first_child() {
        bx.remove(&c);
    }
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    let mpv = &b.mpv;
    let rows = audio_rows(mpv);
    if rows.len() < 2 {
        return false;
    }
    let want = current_aid(mpv);
    block.set(true);
    let p = Rc::clone(player);
    let blk = Rc::clone(block);
    let gl2 = gl.clone();
    let mut first: Option<gtk::CheckButton> = None;
    let mut buttons: Vec<(i64, gtk::CheckButton)> = vec![];

    for r in &rows {
        let btn = gtk::CheckButton::with_label(&r.text);
        if let Some(l) = first.as_ref() {
            btn.set_group(Some(l));
        } else {
            first = Some(btn.clone());
        }
        let id = r.id;
        let p2 = Rc::clone(&p);
        let blk2 = Rc::clone(&blk);
        let gl3 = gl2.clone();
        btn.connect_toggled(move |b| {
            if blk2.get() || !b.is_active() {
                return;
            }
            if let Some(pl) = p2.borrow().as_ref() {
                set_aid(&pl.mpv, id);
            }
            gl3.queue_render();
        });
        buttons.push((id, btn));
    }
    for (_, btn) in &buttons {
        bx.append(btn);
    }
    for (id, btn) in &buttons {
        let active = want == Some(*id);
        btn.set_active(active);
    }
    block.set(false);
    true
}
