//! Wall-clock debounce after fullscreen state reports so duplicate toggles wait out GTK/AppKit transitions.

use std::time::Duration;

/// Idle delay before accepting another fullscreen/unfullscreen request after the window reports a change.
pub const TRANSITION_SETTLE: Duration = Duration::from_millis(380);
