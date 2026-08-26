// Warm continue-grid preload finish: deferred idle completion plus resume-seek retries
// while mpv settles the warm `loadfile`.

const FILE_RESUME_RETRY_MS: &[u64] = &[40, 80, 120, 200, 320, 500, 800, 1200];

fn schedule_file_resume_retries(player: &Rc<RefCell<Option<MpvBundle>>>) {
    if !player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.resume_seek_pending())
    {
        return;
    }
    crate::dvd_vob_log::resume_open_log("schedule file resume retries");
    for &ms in FILE_RESUME_RETRY_MS {
        let p = Rc::clone(player);
        let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(ms), move || {
            if let Some(b) = p.borrow().as_ref() {
                if b.resume_seek_pending() {
                    b.apply_pending_resume();
                }
            }
        });
    }
}
