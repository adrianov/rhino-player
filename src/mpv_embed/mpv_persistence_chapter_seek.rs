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
        self.snapshot_outgoing_if_any();
        let canonical = canonicalize_media_path(path);
        let s = canonical.to_str().ok_or("media path is not valid UTF-8")?;
        self.reset_preview_if_entity_changed_from(&canonical, "load_chapter_seek:chapter_change");
        self.begin_chapter_load_state(
            hold_global,
            chapter_eof,
            local_sec,
            resume_playing,
            &canonical,
        );
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

    /// Snapshot the outgoing item before it is replaced (chapter loads always replace).
    fn snapshot_outgoing_if_any(&self) {
        let shell = self.me_budget_shell_path.borrow();
        if media_probe::shell_media_path(&self.mpv, shell.as_deref()).is_some() {
            self.save_playback_state_for_close();
        }
    }

    /// Pin the held global, flag the cross-chapter scrub, and stash the chapter-local resume.
    fn begin_chapter_load_state(
        &self,
        hold_global: f64,
        chapter_eof: bool,
        local_sec: f64,
        resume_playing: bool,
        canonical: &Path,
    ) {
        self.dvd_hold_global.set(Some(hold_global));
        self.dvd_chain_bar_sync.set(None);
        self.chapter_eof_load.set(chapter_eof);
        self.chapter_scrub_resume.set(true);
        self.begin_chapter_scrub_pause_hold(resume_playing);
        self.pending_resume.set(Some(local_sec.max(0.0)));
        self.set_me_budget_shell_path(canonical);
    }

    /// True when the pending `loadfile` is a same-title DVD chapter advance (EOF auto-load).
    #[must_use]
    pub fn take_chapter_eof_load(&self) -> bool {
        self.chapter_eof_load.replace(false)
    }
}
