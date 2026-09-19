//! Wall-clock debounce after fullscreen state reports so duplicate toggles wait out GTK/AppKit transitions.

use std::time::Duration;

/// Idle delay before accepting another fullscreen/unfullscreen request after the window reports a change.
pub const TRANSITION_SETTLE: Duration = Duration::from_millis(380);

/// Delay between hiding the GTK bars and the legacy cover frame jump: lets GTK relayout and
/// commit a bars-less surface buffer, so the resize rescales video-only content (a laid-out
/// header rescales into a mid-screen ghost). See `macos_legacy_fs::enter_now`.
pub const BAR_HIDE_COMMIT: Duration = Duration::from_millis(120);
