use std::cell::RefCell;

/// Clear a slot after a one-shot idle/timeout callback runs ([`glib::ControlFlow::Break`]).
pub fn finish_glib_source(slot: &RefCell<Option<glib::SourceId>>) {
    if let Ok(mut g) = slot.try_borrow_mut() {
        *g = None;
    }
}

/// Cancel a scheduled GLib idle/timeout; ignores already-removed IDs (stale slots, reused IDs).
///
/// Prefer this over [`glib::SourceId::remove`], which unwraps when the id is stale.
/// One-shot callbacks must call [`finish_glib_source`] (not this) when they return `Break`, or
/// a later cancel will hit a destroyed source and GLib will log `Source ID … was not found`.
pub fn drop_glib_source(slot: &RefCell<Option<glib::SourceId>>) {
    let id = match slot.try_borrow_mut() {
        Ok(mut g) => g.take(),
        Err(_) => return,
    };
    let Some(id) = id else {
        return;
    };
    unsafe {
        let _ = glib::ffi::g_source_remove(id.as_raw());
    }
}
