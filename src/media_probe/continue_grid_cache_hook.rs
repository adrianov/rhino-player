use std::cell::RefCell;
use std::path::Path;

use super::{ContinueGridCache, ContinueSnap};

thread_local! {
    static HOOK: RefCell<Option<ContinueGridCache>> = const { RefCell::new(None) };
}

pub fn attach(cache: ContinueGridCache) {
    HOOK.with(|h| *h.borrow_mut() = Some(cache));
}

pub fn note(entity: &Path, resume_sec: f64, duration_sec: f64) {
    if !(resume_sec.is_finite() && duration_sec.is_finite() && duration_sec > 0.0) {
        return;
    }
    HOOK.with(|h| {
        let Some(ref cache) = *h.borrow() else {
            return;
        };
        let Some(k) = crate::db::history_key(entity) else {
            return;
        };
        cache.borrow_mut().insert(
            k,
            ContinueSnap {
                resume_sec,
                duration_sec,
            },
        );
    });
}
