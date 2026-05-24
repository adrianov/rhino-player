// Per-file source frame rate on `media` rows (Smooth cadence + toolbar readout).

const FPS_SAVE_EPS: f64 = 0.02;
const SNAP_TOL: f64 = 0.035;

const CANONICAL_FPS: [f64; 5] = [
    24000.0 / 1001.0,
    24.0,
    25.0,
    30000.0 / 1001.0,
    30.0,
];

#[must_use]
fn plausible_source_fps_hz(f: f64) -> bool {
    if !f.is_finite() || f <= 0.0 || f >= 120.0 {
        return false;
    }
    CANONICAL_FPS.iter().any(|r| (f - r).abs() < 0.2)
}

/// Map mpv / DB floats to exact broadcast cadence (e.g. 29.970415 → 30000/1001 Hz).
#[must_use]
pub(crate) fn snap_broadcast_fps_hz(f: f64) -> Option<f64> {
    if !plausible_source_fps_hz(f) {
        return None;
    }
    let mut best: Option<f64> = None;
    let mut best_d = f64::MAX;
    for &canon in &CANONICAL_FPS {
        let d = (f - canon).abs();
        if d <= SNAP_TOL && d < best_d {
            best_d = d;
            best = Some(canon);
        }
    }
    best.or(Some(f))
}

/// Cached source fps for [path] (canonical [history_key]).
#[must_use]
pub(crate) fn media_source_fps(path: &std::path::Path) -> Option<f64> {
    let key = history_key(path)?;
    with_conn(|c| {
        let fps: Option<f64> = c
            .query_row(
                "SELECT source_fps_hz FROM media WHERE path = ?1",
                params![&key],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        Ok(fps.and_then(snap_broadcast_fps_hz))
    })
    .flatten()
}

/// Persist source fps once mpv (or DVD heuristics) reports a plausible broadcast rate.
pub(crate) fn media_save_source_fps(path: &std::path::Path, fps: f64) {
    let Some(fps) = snap_broadcast_fps_hz(fps) else {
        return;
    };
    let Some(key) = history_key(path) else {
        return;
    };
    let _ = with_conn(|c| {
        let prior: Option<f64> = c
            .query_row(
                "SELECT source_fps_hz FROM media WHERE path = ?1",
                params![&key],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        if prior.is_some_and(|p| (p - fps).abs() < FPS_SAVE_EPS) {
            return Ok(());
        }
        c.execute(
            "INSERT INTO media (path, source_fps_hz) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET source_fps_hz = excluded.source_fps_hz",
            params![&key, fps],
        )?;
        Ok(())
    });
}

#[cfg(test)]
mod media_source_fps_tests {
    use super::plausible_source_fps_hz;
    use super::snap_broadcast_fps_hz;

    #[test]
    fn accepts_common_broadcast_rates() {
        assert!(plausible_source_fps_hz(30000.0 / 1001.0));
        assert!(plausible_source_fps_hz(24000.0 / 1001.0));
        assert!(plausible_source_fps_hz(25.0));
    }

    #[test]
    fn rejects_outliers() {
        assert!(!plausible_source_fps_hz(60.0));
        assert!(!plausible_source_fps_hz(0.0));
        assert!(!plausible_source_fps_hz(f64::NAN));
    }

    #[test]
    fn snaps_near_ntsc_video_to_exact_cadence() {
        let ntsc = 30000.0 / 1001.0;
        assert!((snap_broadcast_fps_hz(29.970415).unwrap() - ntsc).abs() < 1e-9);
        assert!((snap_broadcast_fps_hz(29.97).unwrap() - ntsc).abs() < 1e-9);
        assert!((snap_broadcast_fps_hz(30.0).unwrap() - 30.0).abs() < 1e-9);
    }
}
