// Cached DVD transport bar state (included from `dvd_vob_timeline.rs`).

impl DvdBarState {
    #[must_use]
    pub fn build(chapter: &Path, live_dur: f64) -> Option<Self> {
        let map = crate::db::load_duration_map();
        Self::build_with_map(chapter, live_dur, &map)
    }

    fn build_with_map(
        chapter: &Path,
        live_dur: f64,
        map: &std::collections::HashMap<String, f64>,
    ) -> Option<Self> {
        let tl = crate::dvd_entity::build_title_timeline(chapter, map, live_dur)?;
        let chapter_labels = chapter_labels_for_timeline(&tl);
        Some(Self { tl, chapter_labels })
    }

    #[must_use]
    pub fn total_sec(&self) -> f64 {
        self.tl.total_sec
    }

    #[must_use]
    pub fn chapter_preview_labels(&self) -> Vec<(f64, String)> {
        self.chapter_labels.clone()
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

    fn mpv_chapter_duration(&self, mpv: &libmpv2::Mpv) -> Option<f64> {
        mpv.get_property::<f64>("duration")
            .ok()
            .filter(|d| d.is_finite() && *d > 0.0)
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
        if let Some(d) = map
            .get(&k)
            .copied()
            .filter(|d| d.is_finite() && *d > 0.0 && *d <= MAX_VOB_DUR_SEC)
        {
            return d;
        }
    }
    0.0
}

fn merge_prior_durs(map: &mut std::collections::HashMap<String, f64>, prior: &DvdBarState) {
    for (i, vob) in prior.tl.vobs.iter().enumerate() {
        let d = prior.tl.chapter_dur_at(i);
        if !(d.is_finite() && d > 0.0 && d <= MAX_VOB_DUR_SEC) {
            continue;
        }
        map.entry(vob.to_string_lossy().into_owned()).or_insert(d);
        if let Ok(c) = std::fs::canonicalize(vob) {
            map.entry(c.to_string_lossy().into_owned()).or_insert(d);
        }
    }
}

/// Rebuild when the bar is missing or still capped at the open `.vob` mpv `duration`.
pub fn maybe_refresh_dvd_bar(
    slot: &std::cell::RefCell<Option<DvdBarState>>,
    mpv: &libmpv2::Mpv,
    shell: Option<&Path>,
) {
    let Some(chapter) = open_dvd_chapter_path(mpv, shell) else {
        return;
    };
    let Some(vobs) = crate::dvd_entity::title_chapter_paths(&chapter) else {
        return;
    };
    if vobs.len() <= 1 {
        return;
    }
    let live = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    let on_disk_n = vobs.len();
    let open = open_dvd_chapter_path(mpv, shell);
    let stale = slot.borrow().as_ref().is_none_or(|b| {
        b.tl.vobs.len() < on_disk_n
            || (live > 0.0 && b.total_sec() <= live * 1.05)
            || b.total_sec() > live * on_disk_n as f64 * 1.5
            || open.as_ref().is_some_and(|p| b.tl.index_of(p).is_none())
    });
    if stale {
        refresh_dvd_bar(slot, mpv, shell);
    }
}

/// Before `.vob` EOF advance: rebuild when the bar still looks like a single-file title.
pub fn refresh_dvd_bar_at_chapter_eof(
    slot: &std::cell::RefCell<Option<DvdBarState>>,
    mpv: &libmpv2::Mpv,
    shell: Option<&Path>,
) {
    let Some(chapter) = open_dvd_chapter_path(mpv, shell) else {
        return;
    };
    if !chapter_local_at_eof(mpv) {
        return;
    }
    let on_disk_n = crate::dvd_entity::title_chapter_paths(&chapter)
        .map(|c| c.len())
        .unwrap_or(0);
    if on_disk_n <= 1 {
        return;
    }
    let stale = slot.borrow().as_ref().is_none_or(|b| {
        b.tl.vobs.len() < on_disk_n
            || b.tl.next_chapter_after(&chapter).is_none()
            || (b.mpv_chapter_duration(mpv).is_some_and(|live| {
                live > 0.0 && b.total_sec() <= live * 1.05
            }))
            || b.mpv_chapter_duration(mpv).is_some_and(|live| {
                b.tl
                    .index_of(&chapter)
                    .is_some_and(|i| live + 0.5 < b.tl.chapter_dur_at(i))
            })
    });
    if stale {
        refresh_dvd_bar(slot, mpv, shell);
    }
}

/// True when sibling-folder EOF advance may run (title finished, not mid-`.vob` tail).
pub(crate) fn title_eof_for_sibling_advance(
    mpv: &libmpv2::Mpv,
    bar: Option<&DvdBarState>,
    bar_dur: f64,
    bar_pos: f64,
) -> bool {
    if bar_dur > 0.0 && (bar_dur - bar_pos) > crate::app::TICK_EOF_TAIL_SEC {
        return false;
    }
    if let Some(bar) = bar {
        if let Some(ch) = open_dvd_chapter_path(mpv, None) {
            if bar.tl.next_chapter_after(&ch).is_some() {
                return false;
            }
        }
    }
    if bar_dur > 0.0 && (bar_dur - bar_pos) <= crate::app::TICK_EOF_TAIL_SEC {
        return true;
    }
    bar.is_some() && chapter_local_at_eof(mpv)
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
    let mut map = crate::db::load_duration_map();
    let prior_meta = {
        let guard = slot.borrow();
        if let Some(old) = guard.as_ref() {
            merge_prior_durs(&mut map, old);
            Some((old.total_sec(), old.tl.vobs.len()))
        } else {
            None
        }
    };
    let bar = DvdBarState::build_with_map(&chapter, live, &map);
    if live == 0.0 {
        if let (Some(ref new_b), Some((old_total, old_n))) = (&bar, prior_meta) {
            if new_b.tl.vobs.len() == old_n
                && old_total > 60.0
                && new_b.total_sec() > old_total * 1.5
            {
                crate::dvd_vob_log::dvd_seek_log(format!(
                    "refresh_dvd_bar: keep prior total={old_total:.1}s (new={:.1}s live=0)",
                    new_b.total_sec()
                ));
                return;
            }
        }
    }
    if let Some(ref b) = bar {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "refresh_dvd_bar: total={:.1}s vobs={} on_disk={on_disk_n} file={}",
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
