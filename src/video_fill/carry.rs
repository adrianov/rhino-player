//! Sibling-advance carry: the previous video's fill intent, bound to the exact
//! sibling target for one media change (`docs/features/32-fill-screen.md`).

use std::cell::RefCell;

thread_local! {
    static FILL_CARRY: RefCell<Option<std::path::PathBuf>> = const { RefCell::new(None) };
}

/// Bind the pending media change to [target]: a sibling transition carries the current
/// fill intent to that exact video only. The marker dies at the next
/// `reset_preferred` regardless of outcome, so a failed or abandoned sibling load
/// cannot leak the carry onto an unrelated open.
pub(crate) fn request_fill_carry(target: &std::path::Path) {
    FILL_CARRY.with(|c| *c.borrow_mut() = Some(target.to_path_buf()));
}

/// Consume the carry target set by [`request_fill_carry`].
pub(super) fn take_fill_carry_target() -> Option<std::path::PathBuf> {
    FILL_CARRY.with(RefCell::take)
}

/// Carry verdict for one media reset: the marker applies only when the media that
/// actually opened is the bound sibling target, and only carries an active intent.
pub(super) fn carry_applies(
    carry_target: Option<std::path::PathBuf>,
    opened: Option<std::path::PathBuf>,
    prev_pref: bool,
) -> bool {
    carry_target
        .zip(opened)
        .is_some_and(|(t, p)| crate::video_ext::paths_same_file(&t, &p))
        && prev_pref
}

#[cfg(test)]
mod carry_tests {
    use super::carry_applies;

    fn p(s: &str) -> Option<std::path::PathBuf> {
        Some(std::path::PathBuf::from(s))
    }

    #[test]
    fn carry_needs_marker_target_and_active_intent() {
        // Marker matches the opened media and fill was on: carry.
        assert!(carry_applies(p("/v/a.mkv"), p("/v/a.mkv"), true));
        // Fitted intent carries as nothing (fitted is the default).
        assert!(!carry_applies(p("/v/a.mkv"), p("/v/a.mkv"), false));
        // Opened media is not the bound target: unrelated open stays fitted.
        assert!(!carry_applies(p("/v/a.mkv"), p("/v/other.mkv"), true));
        // No marker (unrelated open) or media never opened: no carry.
        assert!(!carry_applies(None, p("/v/a.mkv"), true));
        assert!(!carry_applies(p("/v/a.mkv"), None, true));
    }
}
