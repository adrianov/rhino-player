/// State for `maybe_advance_sibling_on_eof`: one-shot guard per logical end.
struct SiblingEofState {
    done: Cell<bool>,
    /// Last canonical path for which `nav_sensitivity` was computed; avoids `prev` / `next` directory walks every 200ms.
    nav_key: RefCell<Option<PathBuf>>,
    nav_can_prev: Cell<bool>,
    nav_can_next: Cell<bool>,
    /// Min/max `time-pos` since the last load; sibling EOF requires playing into the tail, not opening with resume near end.
    pos_min: Cell<f64>,
    pos_max: Cell<f64>,
    pos_tracked: Cell<bool>,
    /// Incomplete `.dctmp` EOF pause / resume (shared with [PlayToggleCtx]).
    incomplete_hold: Rc<crate::incomplete_download_eof::IncompleteEofHold>,
}

/// Minimum seconds of position movement before tail EOF counts as natural playback (not resume-at-end open).
const SIBLING_PLAY_SPAN_MIN: f64 = 1.0;

impl SiblingEofState {
    /// Prev/next button sensitivity for `cur`. Reuses cached fs work while the file path is unchanged.
    fn nav_sensitivity(&self, cur: &Path) -> (bool, bool) {
        let Some(can) = nav_canonical_key(cur) else {
            *self.nav_key.borrow_mut() = None;
            return (false, false);
        };
        {
            let k = self.nav_key.borrow();
            if k.as_ref() == Some(&can) {
                return (self.nav_can_prev.get(), self.nav_can_next.get());
            }
        }
        let (cp, cn) = sibling_nav_flags(cur);
        *self.nav_key.borrow_mut() = Some(can);
        self.nav_can_prev.set(cp);
        self.nav_can_next.set(cn);
        (cp, cn)
    }

    fn clear_nav_sensitivity(&self) {
        *self.nav_key.borrow_mut() = None;
    }

    fn reset_playback_span(&self) {
        self.pos_tracked.set(false);
        self.pos_min.set(0.0);
        self.pos_max.set(0.0);
        self.incomplete_hold.reset();
    }

    fn note_transport_pos(&self, pos: f64) {
        if !pos.is_finite() || pos < 0.0 {
            return;
        }
        if !self.pos_tracked.get() {
            self.pos_min.set(pos);
            self.pos_max.set(pos);
            self.pos_tracked.set(true);
            return;
        }
        if pos < self.pos_min.get() {
            self.pos_min.set(pos);
        }
        if pos > self.pos_max.get() {
            self.pos_max.set(pos);
        }
    }

    /// True after natural playback into the title tail (or mpv EOF), not when opening with resume already near end.
    fn played_into_tail(&self, dur: f64, eof_reached: bool) -> bool {
        if eof_reached {
            return true;
        }
        if dur <= 0.0 || !self.pos_tracked.get() {
            return false;
        }
        self.pos_max.get() - self.pos_min.get() > SIBLING_PLAY_SPAN_MIN
            && self.pos_max.get() >= (dur - crate::media_probe::NEAR_END_SEC).max(0.0)
    }
}

/// Canonical cache key for sibling-nav state: `None` when `cur` is not a plain existing file.
fn nav_canonical_key(cur: &Path) -> Option<PathBuf> {
    if !cur.is_file() {
        return None;
    }
    std::fs::canonicalize(cur).ok()
}

/// Fresh fs walk for prev/next availability in folder/sibling order.
fn sibling_nav_flags(cur: &Path) -> (bool, bool) {
    (
        sibling_advance::prev_before_current(cur).is_some(),
        sibling_advance::next_after_eof(cur).is_some(),
    )
}
