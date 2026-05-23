// Cached DVD transport bar state (included from `dvd_vob_timeline.rs`).

impl DvdBarState {
    #[must_use]
    pub fn build(chapter: &Path, live_dur: f64) -> Option<Self> {
        let mut tl = DvdVobTimeline::from_chapter_ifo(chapter).or_else(|| {
            let map = crate::db::load_duration_map();
            DvdVobTimeline::from_chapter(chapter, &map, chapter, live_dur)
        })?;
        if let Some(on_disk) = crate::dvd_entity::title_chapter_paths(chapter) {
            tl.expand_on_disk_chapters(&on_disk);
        }
        if live_dur > 0.0 {
            tl.apply_live_chapter_dur(chapter, live_dur);
        }
        (tl.total_sec > 0.0).then_some(Self { tl })
    }

    #[must_use]
    pub fn total_sec(&self) -> f64 {
        self.tl.total_sec
    }

    #[must_use]
    pub fn chapter_marks(&self) -> Vec<(f64, String)> {
        self.tl.chapter_mark_times()
    }

    #[must_use]
    pub fn resolve_global(&self, global: f64) -> (usize, f64) {
        self.tl.resolve_global(global)
    }

    pub fn path_at(&self, index: usize) -> Option<&std::path::Path> {
        self.tl.path_at(index)
    }

    #[must_use]
    pub fn global_pos(&self, chapter: &std::path::Path, local_pos: f64) -> f64 {
        self.tl.global_pos(chapter, local_pos)
    }

    #[must_use]
    pub fn chapter_dur_at(&self, index: usize) -> f64 {
        self.tl.chapter_dur_at(index)
    }

    /// Title-wide seek-bar position: honor [MpvBundle::dvd_hold_global] only while it matches live time.
    #[must_use]
    pub fn transport_global_pos(
        &self,
        b: &crate::mpv_embed::MpvBundle,
        chapter: &Path,
        local_pos: f64,
    ) -> f64 {
        let computed = self.global_pos(chapter, local_pos);
        match b.dvd_hold_global.get() {
            Some(h) if (h - computed).abs() <= crate::app::TICK_EOF_TAIL_SEC => h,
            Some(_) => {
                b.dvd_hold_global.set(None);
                computed
            }
            None => computed,
        }
    }
}

pub(crate) fn dur_from_map(
    map: &std::collections::HashMap<String, f64>,
    path: &Path,
) -> f64 {
    let mut keys = vec![path.to_string_lossy().into_owned()];
    if let Ok(c) = std::fs::canonicalize(path) {
        keys.push(c.to_string_lossy().into_owned());
    }
    for k in keys {
        if let Some(d) = map.get(&k).copied().filter(|d| d.is_finite() && *d > 0.0) {
            return d;
        }
    }
    0.0
}

/// Rebuild when the bar is missing or still capped at the open chapter's mpv `duration`.
pub fn maybe_refresh_dvd_bar(
    slot: &std::cell::RefCell<Option<DvdBarState>>,
    mpv: &libmpv2::Mpv,
    shell: Option<&Path>,
) {
    let Some(chapter) = open_dvd_chapter_path(mpv, shell) else {
        return;
    };
    let Some(chapters) = crate::dvd_entity::title_chapter_paths(&chapter) else {
        return;
    };
    if chapters.len() <= 1 {
        return;
    }
    let live = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    let on_disk_n = chapters.len();
    let open = open_dvd_chapter_path(mpv, shell);
    let stale = slot.borrow().as_ref().is_none_or(|b| {
        b.tl.vobs.len() < on_disk_n
            || (live > 0.0 && b.total_sec() <= live * 1.05)
            || open.as_ref().is_some_and(|p| b.tl.index_of(p).is_none())
    });
    if stale {
        refresh_dvd_bar(slot, mpv, shell);
    }
}

/// Rebuild cached bar state after `FileLoaded` / path change (not on every transport tick).
pub fn refresh_dvd_bar(
    slot: &std::cell::RefCell<Option<DvdBarState>>,
    mpv: &libmpv2::Mpv,
    shell: Option<&Path>,
) {
    let Some(chapter) = open_dvd_chapter_path(mpv, shell) else {
        *slot.borrow_mut() = None;
        return;
    };
    if !crate::playback_entity::PlaybackEntity::resolve(&chapter).has_unified_timeline() {
        *slot.borrow_mut() = None;
        return;
    }
    let live = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    let on_disk_n = crate::dvd_entity::title_chapter_paths(&chapter)
        .map(|c| c.len())
        .unwrap_or(0);
    let bar = DvdBarState::build(&chapter, live);
    if let Some(ref b) = bar {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "refresh_dvd_bar: total={:.1}s chapters={} on_disk={on_disk_n} file={}",
            b.total_sec(),
            b.tl.vobs.len(),
            chapter.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        ));
    } else {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "refresh_dvd_bar: build failed for {}",
            chapter.display()
        ));
    }
    *slot.borrow_mut() = bar;
}

fn open_dvd_chapter_path(mpv: &libmpv2::Mpv, shell: Option<&Path>) -> Option<std::path::PathBuf> {
    let path = crate::media_probe::local_file_from_mpv(mpv).or_else(|| {
        shell.and_then(|p| std::fs::canonicalize(p).ok().or_else(|| Some(p.to_path_buf())))
    })?;
    crate::video_ext::is_dvd_vob_path(&path).then_some(path)
}
