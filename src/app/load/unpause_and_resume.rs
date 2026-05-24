const RESUME_AFTER_UNPAUSE_MS: &[u64] = &[0, 80, 200, 500, 1000];

/// Unpause first so optical demuxers can expose `duration`, then seek resume when ready.
fn unpause_and_finish_resume(player: &Rc<RefCell<Option<MpvBundle>>>) {
    if let Some(b) = player.borrow().as_ref() {
        let _ = b.mpv.set_property("pause", false);
    }
    schedule_resume_after_unpause(Rc::clone(player));
}

fn schedule_resume_after_unpause(player: Rc<RefCell<Option<MpvBundle>>>) {
    for &ms in RESUME_AFTER_UNPAUSE_MS {
        let p = Rc::clone(&player);
        let _ = glib::timeout_add_local_once(Duration::from_millis(ms), move || {
            let g = p.borrow();
            let Some(b) = g.as_ref() else {
                return;
            };
            // Later retries only run while a stashed seek is still pending; ms=0 may still warm-open from DB.
            if ms > 0 && !b.resume_seek_pending() {
                return;
            }
            let _ = b.ensure_resume_before_unpause();
            if !b.resume_seek_pending() {
                return;
            }
            transport_nudge_tick();
        });
    }
}
