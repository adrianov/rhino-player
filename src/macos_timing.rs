//! Darwin-only delays so GTK/chrome does not nest inside `_NSExitFullScreenTransitionController`.

use std::time::Duration;

/// Wall-clock wait after fullscreen state changes before driving `unfullscreen`, window restore, or
/// traffic-light visibility — avoids `_NSThemeFrame` titlebar recursion (macOS 26+).
pub const FULLSCREEN_TRANSITION_SETTLE: Duration = Duration::from_millis(380);
