impl MpvBundle {

/// Cross-chapter DVD seek: `loadfile` with chapter-local resume (not entity-global remap).
pub fn load_chapter_seek(
    &self,
    path: &Path,
    local_sec: f64,
    hold_global: f64,
    resume_playing: bool,
    chapter_eof: bool,
) -> Result<(), String> {
    {
        let shell = self.me_budget_shell_path.borrow();
        let outgoing = media_probe::shell_media_path(&self.mpv, shell.as_deref());
        if outgoing.is_some() {
            self.save_playback_state_for_close();
        }
    }
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canonical.to_str().ok_or("media path is not valid UTF-8")?;
    let prev_shell = self.me_budget_shell_path.borrow().clone();
    if prev_shell
        .as_ref()
        .is_some_and(|p| crate::preview_debug::open_target_entity_changed(p, &canonical))
    {
        crate::seek_bar_preview::reset_on_main_media_change_from("load_chapter_seek:chapter_change");
    }
    self.dvd_hold_global.set(Some(hold_global));
    self.dvd_chain_bar_sync.set(None);
    self.chapter_eof_load.set(chapter_eof);
    self.chapter_scrub_resume.set(true);
    self.begin_chapter_scrub_pause_hold(resume_playing);
    self.pending_resume.set(Some(local_sec.max(0.0)));
    self.set_me_budget_shell_path(&canonical);
    crate::video_pref::strip_vapoursynth_before_replace_media(self);
    crate::dvd_vob_log::dvd_seek_log(format!(
        "load_chapter_seek file={} local={local_sec:.2} hold_global={hold_global:.2}",
        canonical.display()
    ));
    if let Err(e) = self.mpv.command("loadfile", &[s, "replace"]) {
        self.abort_chapter_load(true);
        return Err(format!("{e:?}"));
    }
    Ok(())
}

/// True when the pending `loadfile` is a same-title DVD chapter advance (EOF auto-load).
#[must_use]
pub fn take_chapter_eof_load(&self) -> bool {
    self.chapter_eof_load.replace(false)
}

}
