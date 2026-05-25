// Drop stale seek-bar preview state when main playback loads another file.

thread_local! {
    static SEEK_PREVIEW: RefCell<Option<Rc<SeekPreviewState>>> = const { RefCell::new(None) };
}

pub fn register(st: Rc<SeekPreviewState>) {
    SEEK_PREVIEW.with(|slot| *slot.borrow_mut() = Some(st));
}

/// Invalidate cached preview media and hide the overlay (safe before/after main `loadfile`).
pub fn reset_on_main_media_change_from(from: &'static str) {
    SEEK_PREVIEW.with(|slot| {
        let guard = slot.borrow();
        let Some(st) = guard.as_ref() else {
            crate::preview_debug::info(format!("reset from {from} (preview not wired yet)"));
            return;
        };
        let reschedule = st.container.is_visible()
            && st.enabled.get()
            && st.last_xy.borrow().is_some();
        let st = Rc::clone(st);
        drop(guard);
        st.reset_for_new_media(from);
        if reschedule {
            crate::preview_debug::info(format!("reset from {from}: re-schedule seek (hover active)"));
            schedule_preview_seek(st);
        }
    });
}
