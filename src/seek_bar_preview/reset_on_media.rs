// Drop stale seek-bar preview state when main playback loads another file.

thread_local! {
    static SEEK_PREVIEW: RefCell<Option<Rc<SeekPreviewState>>> = const { RefCell::new(None) };
}

pub fn register(st: Rc<SeekPreviewState>) {
    SEEK_PREVIEW.with(|slot| *slot.borrow_mut() = Some(st));
}

/// Invalidate cached preview media and hide the overlay (safe before/after main `loadfile`).
pub fn reset_on_main_media_change_from(from: &'static str) {
    let Some(st) = preview_state() else {
        crate::preview_debug::info(format!("reset from {from} (preview not wired yet)"));
        return;
    };
    let reschedule = should_reschedule_after_reset(&st);
    st.reset_for_new_media(from);
    if reschedule {
        crate::preview_debug::info(format!("reset from {from}: re-schedule seek (hover active)"));
        schedule_preview_seek(st);
    }
}

/// Continue strip shown again — drop any framed preview left from playback.
pub fn dismiss_for_browse() {
    let Some(st) = preview_state() else {
        return;
    };
    st.serial.set(st.serial.get().wrapping_add(1));
    crate::glib_source_drop::drop_glib_source(st.deb.as_ref());
    crate::glib_source_drop::drop_glib_source(st.pump.as_ref());
    *st.last_xy.borrow_mut() = None;
    st.hide();
}

fn preview_state() -> Option<Rc<SeekPreviewState>> {
    SEEK_PREVIEW.with(|slot| slot.borrow().clone())
}

fn should_reschedule_after_reset(st: &SeekPreviewState) -> bool {
    st.container.is_visible()
        && st.enabled.get()
        && !st.recent_visible.get()
        && st.last_xy.borrow().is_some()
}
