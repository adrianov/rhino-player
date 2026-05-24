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
            if let Some(b) = p.borrow().as_ref() {
                let _ = b.ensure_resume_before_unpause();
            }
            transport_nudge_tick();
        });
    }
}
