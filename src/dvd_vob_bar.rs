// Cached DVD transport bar state (included from `dvd_vob_timeline.rs`).

impl DvdBarState {
    #[must_use]
    pub fn build(chapter: &Path, live_dur: f64) -> Option<Self> {
        Self::build_with_map(chapter, live_dur, &crate::db::load_duration_map())
    }

    pub(crate) fn build_with_map(
        chapter: &Path,
        live_dur: f64,
        map: &std::collections::HashMap<String, f64>,
    ) -> Option<Self> {
        Self::build_with_map_opts(
            chapter,
            live_dur,
            map,
            crate::dvd_entity::TimelineBuildOpts::PLAYBACK,
        )
    }

    pub(crate) fn build_with_map_opts(
        chapter: &Path,
        live_dur: f64,
        map: &std::collections::HashMap<String, f64>,
        opts: crate::dvd_entity::TimelineBuildOpts,
    ) -> Option<Self> {
        let tl = crate::dvd_entity::build_title_timeline_with(chapter, map, live_dur, opts)?;
        Some(Self {
            chapter_labels: chapter_labels_for_timeline(&tl),
            tl,
        })
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

    fn mpv_chapter_duration(&self, mpv: &libmpv2::Mpv) -> Option<f64> {
        mpv.get_property::<f64>("duration")
            .ok()
            .filter(|d| d.is_finite() && *d > 0.0)
    }
}

pub(crate) fn dur_from_map(map: &std::collections::HashMap<String, f64>, path: &Path) -> f64 {
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

/// True when the cached bar should be rebuilt (missing, incomplete title, or single-file total).
pub(crate) fn bar_cache_stale(
    bar: &DvdBarState,
    live: f64,
    on_disk_n: usize,
    open: Option<&Path>,
) -> bool {
    bar.tl.vobs.len() < on_disk_n
        || (on_disk_n > 1 && live > 0.0 && bar.total_sec() <= live * 1.05)
        || open.is_some_and(|p| bar.tl.index_of(p).is_none())
}

pub fn maybe_refresh_dvd_bar(
    slot: &std::rc::Rc<std::cell::RefCell<Option<DvdBarState>>>,
    mpv: &libmpv2::Mpv,
    shell: Option<&Path>,
) {
    let Some(chapter) = open_dvd_chapter_path(mpv, shell) else {
        return;
    };
    let Some(vobs) = crate::dvd_entity::timeline_chapter_paths(&chapter) else {
        return;
    };
    if vobs.len() <= 1 {
        return;
    }
    let live = live_vob_duration(mpv);
    let on_disk_n = vobs.len();
    let open = open_dvd_chapter_path(mpv, shell);
    let stale = slot.borrow().as_ref().map_or(true, |b| {
        bar_cache_stale(b, live, on_disk_n, open.as_deref())
    });
    if stale {
        refresh_dvd_bar(slot, mpv, shell);
    }
}

/// Live open-chapter length clamped into VOB range; `0.0` when mpv reports none.
fn live_vob_duration(mpv: &libmpv2::Mpv) -> f64 {
    mpv.get_property::<f64>("duration")
        .ok()
        .map(crate::dvd_vob_timeline::clamp_vob_duration)
        .unwrap_or(0.0)
}

include!("dvd_sibling_eof_advance.rs");

include!("dvd_vob_bar_refresh.rs");

include!("dvd_vob_probe_tail.rs");

fn open_dvd_chapter_path(mpv: &libmpv2::Mpv, shell: Option<&Path>) -> Option<std::path::PathBuf> {
    crate::playback_entity::unified_timeline_chapter(mpv, shell)
}
