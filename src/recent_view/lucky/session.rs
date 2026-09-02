// Lucky session: shown handful, seen titles, reserved next + still warming.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::gap::fill_lucky_gap;
use super::titles::{group_index, openable_set, retarget_lists, titles_from_groups};
use super::{keep_openable, take_ready_then_next, NeighbourEntry};

/// Per-window I'm Feeling Lucky state (feature 33).
pub(crate) struct LuckySession {
    shown: RefCell<Option<Vec<PathBuf>>>,
    seen: RefCell<HashSet<String>>,
    next: RefCell<Option<Vec<PathBuf>>>,
    /// Title id → listing paths; filled once from the session neighbour index.
    groups: RefCell<Option<HashMap<String, Vec<PathBuf>>>>,
}

impl LuckySession {
    pub(crate) fn new() -> Self {
        Self {
            shown: RefCell::new(None),
            seen: RefCell::default(),
            next: RefCell::default(),
            groups: RefCell::default(),
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.shown.borrow().is_some()
    }

    pub(crate) fn deactivate(&self) {
        self.shown.borrow_mut().take();
    }

    pub(crate) fn strip_hits(&self, index: &[NeighbourEntry]) -> Option<Vec<PathBuf>> {
        Some(keep_openable(self.shown.borrow().as_ref()?, index))
    }

    /// Re-pick continue-or-first for each title already on the strip or reserved.
    pub(crate) fn retarget(&self, index: &[NeighbourEntry]) {
        if self.shown.borrow().is_none() {
            return;
        }
        self.ensure_groups(index);
        let groups = self.groups.borrow();
        let Some(groups) = groups.as_ref() else {
            return;
        };
        let tpos = crate::db::load_time_pos_map();
        let durs = crate::db::load_duration_map();
        retarget_lists(
            &mut self.shown.borrow_mut(),
            &mut self.next.borrow_mut(),
            groups,
            &openable_set(index),
            &tpos,
            &durs,
        );
    }

    pub(crate) fn roll(&self, index: &[NeighbourEntry], max: usize) {
        let tpos = crate::db::load_time_pos_map();
        let durs = crate::db::load_duration_map();
        let titles = self.current_titles(index, &tpos, &durs);
        let (picks, reserved) = {
            let mut ready = self.next.borrow_mut();
            let mut seen = self.seen.borrow_mut();
            take_ready_then_next(&mut ready, titles, max, &mut seen, index)
        };
        *self.shown.borrow_mut() = Some(picks);
        *self.next.borrow_mut() = reserved;
    }

    /// Reserved next handful, still openable, not already in `paths`.
    pub(crate) fn append_warm(&self, paths: &mut Vec<PathBuf>, index: &[NeighbourEntry]) {
        let Some(next) = self.next.borrow().clone() else {
            return;
        };
        for p in warm_extra(&next, index) {
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
    }

    /// After trash or Remove: drop the gone path and fill that slot from the reserved next handful.
    /// `false` when lucky is inactive or `gone` is not on the shown handful.
    pub(crate) fn refill_slot(&self, gone: &Path, index: &[NeighbourEntry]) -> bool {
        let mut shown = self.shown.borrow_mut();
        let Some(lucky) = shown.as_mut() else {
            return false;
        };
        if !lucky.iter().any(|p| super::same_shown(p, gone)) {
            return false;
        }
        let mut next = self.next.borrow_mut();
        let mut seen = self.seen.borrow_mut();
        fill_lucky_gap(lucky, &mut next, gone, index, &mut seen);
        true
    }

    fn current_titles(
        &self,
        index: &[NeighbourEntry],
        tpos: &HashMap<String, f64>,
        durs: &HashMap<String, f64>,
    ) -> Vec<(String, PathBuf)> {
        self.ensure_groups(index);
        let groups = self.groups.borrow();
        let Some(groups) = groups.as_ref() else {
            return Vec::new();
        };
        titles_from_groups(groups, &openable_set(index), tpos, durs)
    }

    fn ensure_groups(&self, index: &[NeighbourEntry]) {
        if self.groups.borrow().is_some() || index.is_empty() {
            return;
        }
        *self.groups.borrow_mut() = Some(group_index(index));
    }
}

fn warm_extra(next: &[PathBuf], index: &[NeighbourEntry]) -> Vec<PathBuf> {
    if index.is_empty() {
        next.to_vec()
    } else {
        keep_openable(next, index)
    }
}
