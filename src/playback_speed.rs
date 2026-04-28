//! Header list + mpv `speed` in fixed steps. See `docs/features/28-playback-speed.md`.

use gtk::ListBox;
use libmpv2::Mpv;

/// Fastest fixed step: matches mpv `scaletempo2` default `max-speed` when `--audio-pitch-correction` is on.
pub const MAX_FIXED_SPEED: f64 = 8.0;

/// Supported `speed` values (order matches the header `ListBox` row index).
pub const SPEEDS: [f64; 4] = [1.0, 1.5, 2.0, MAX_FIXED_SPEED];

const EPS: f64 = 0.01;

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

/// Value for list row [index] (0..=3).
pub fn value_at(i: u32) -> f64 {
    SPEEDS.get(i as usize).copied().unwrap_or(1.0)
}

/// Snap the header [ListBox] to current **mpv** `speed`; [flag] blocks `row-activated` when updating.
/// Returns [Some] **canonical** speed if [mpv] `speed` was changed to a [SPEEDS] step (caller may resync
/// VapourSynth env + [vf]); [None] if it was already on a step.
pub fn sync_list(mpv: &Mpv, flag: &std::cell::Cell<bool>, list: &ListBox) -> Option<f64> {
    let s = mpv.get_property::<f64>("speed").unwrap_or(1.0);
    let (i, canon) = nearest(s);
    let changed = if (s - canon).abs() > EPS {
        let _ = mpv.set_property("speed", canon);
        true
    } else {
        false
    };
    flag.set(true);
    if let Some(row) = list.row_at_index(i as i32) {
        list.select_row(Some(&row));
    }
    flag.set(false);
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
        assert_eq!(nearest(8.0), (3, 8.0));
        assert_eq!(nearest(1.45), (1, 1.5));
        assert_eq!(nearest(1.25), (0, 1.0));
        assert_eq!(nearest(7.0), (3, 8.0));
    }
}
