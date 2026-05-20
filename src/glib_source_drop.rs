use std::cell::RefCell;

/// Cancel a scheduled GLib idle/timeout; ignores already-removed IDs (stale slots, reused IDs).
///
/// Prefer this over [`glib::SourceId::remove`], which unwraps when the id is stale.
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
