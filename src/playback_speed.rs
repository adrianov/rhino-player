//! Header list + mpv `speed` in fixed steps. See `docs/features/28-playback-speed.md`.

use gtk::{Label, ListBox};
use libmpv2::Mpv;

/// Fastest fixed step: matches mpv `scaletempo2` default `max-speed` when `--audio-pitch-correction` is on.
pub const MAX_FIXED_SPEED: f64 = 8.0;

/// Supported `speed` values (popover row order matches this array).
pub const SPEEDS: [f64; 9] = [1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, MAX_FIXED_SPEED];

const EPS: f64 = 0.01;

/// Single-line × rate for rows and header readout (matches popover formatting).
#[must_use]
pub fn format_step(v: f64) -> String {
    format!("{v:.1}×")
}

/// Updates the caption [`gtk::Label`] inside the speed header [`gtk::MenuButton`].
#[inline]
pub fn stamp_speed_readout(l: &Label, canon: f64) {
    l.set_label(&format_step(canon));
}

/// Force **1.0×** when mpv speed differs (folder auto-advance after faster playback).
pub fn force_normal(mpv: &Mpv) {
    let s = mpv.get_property::<f64>("speed").unwrap_or(1.0);
    if (s - 1.0).abs() > EPS {
        let _ = mpv.set_property("speed", 1.0);
    }
}

/// Nearest [SPEEDS] entry and (if needed) a canonical [speed] for mpv.
pub fn nearest(mpv_speed: f64) -> (u32, f64) {
    let (i, &v) = SPEEDS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (mpv_speed - *a)
                .abs()
                .partial_cmp(&(mpv_speed - *b).abs())
                .unwrap()
        })
        .unwrap();
    (i as u32, v)
}

/// Value for list row [`index`] in `0..SPEEDS.len()`.
pub fn value_at(i: u32) -> f64 {
    SPEEDS.get(i as usize).copied().unwrap_or(1.0)
}

/// Snap the header [ListBox] and compact readout to current **mpv** `speed`; [flag] blocks
/// `row-selected` when updating.
/// Returns [Some] **canonical** speed if [mpv] `speed` was changed to a [SPEEDS] step (caller may resync
/// VapourSynth env + [vf]); [None] if it was already on a step.
pub fn sync_list(
    mpv: &Mpv,
    flag: &std::rc::Rc<std::cell::Cell<bool>>,
    list: &ListBox,
    readout: &Label,
) -> Option<f64> {
    let s = mpv.get_property::<f64>("speed").unwrap_or(1.0);
    let (i, canon) = nearest(s);
    let changed = if (s - canon).abs() > EPS {
        let _ = mpv.set_property("speed", canon);
        true
    } else {
        false
    };
    stamp_speed_readout(readout, canon);
    flag.set(true);
    if let Some(row) = list.row_at_index(i as i32) {
        list.select_row(Some(&row));
    }
    let f = std::rc::Rc::clone(flag);
    let _ = glib::idle_add_local_once(move || f.set(false));
    if changed {
        Some(canon)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_snaps() {
        assert_eq!(nearest(1.0), (0, 1.0));
        assert_eq!(nearest(1.5), (1, 1.5));
        assert_eq!(nearest(2.0), (2, 2.0));
        assert_eq!(nearest(8.0), (8, 8.0));
        assert_eq!(nearest(1.45), (1, 1.5));
        assert_eq!(nearest(1.25), (0, 1.0));
        assert_eq!(nearest(7.0), (7, 7.0));
        assert_eq!(nearest(7.49), (7, 7.0));
        assert_eq!(nearest(7.51), (8, 8.0));
    }
}
