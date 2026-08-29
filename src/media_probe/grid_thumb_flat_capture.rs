//! Session mark for flat continue-grid stills: after nearby-seek capture, accept a still-flat BLOB
//! so backfill workers do not reject and respin forever.
//! Process-wide mutex — workers and UI share one set (not thread-local).

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

fn flat_capture_done() -> &'static Mutex<HashSet<String>> {
    static DONE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    DONE.get_or_init(|| Mutex::new(HashSet::new()))
}

fn lock_done() -> std::sync::MutexGuard<'static, HashSet<String>> {
    flat_capture_done()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(super) fn mark_done(db_key: &str) {
    lock_done().insert(db_key.to_string());
}

fn is_done(db_key: &str) -> bool {
    lock_done().contains(db_key)
}

/// Drop flat fills until a capture attempt finished this process; otherwise keep `b`.
pub(super) fn take_unless_flat_pending(db_key: &str, b: Vec<u8>) -> Option<Vec<u8>> {
    if crate::thumb_texture::thumb_webp_is_flat_fill(&b) && !is_done(db_key) {
        return None;
    }
    Some(b)
}
