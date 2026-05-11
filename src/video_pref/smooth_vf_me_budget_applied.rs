/// libmpv+VapourSynth often **reuse** an interpreter instance when **`vf vapoursynth:`** options are unchanged.
/// Refreshing **`RHINO_SMOOTH_MAX_AREA` alone cannot retune a warm worker graph — **`vf clr`/`vf add`** is required when ME px² changes.
/// Rhino records which clamped **`video_smooth_max_area`** px² and **`media`** identity (**`history_key`**) were last
/// **successfully rebuilt** (`vf clr`/`vf add`) so [vf_smooth_matches_prefs] skips only when SQLite, env, and the open
/// **local file** stay in sync (**`LAST_BUNDLED_ME_BUDGET_APPLIED`** + **`LAST_BUNDLED_MEDIA_KEY`**).
const UNSET: u64 = u64::MAX;
static LAST_BUNDLED_ME_BUDGET_APPLIED: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(UNSET);
static LAST_BUNDLED_MEDIA_KEY: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub(crate) fn forget_bundled_me_budget_vf_apply() {
    LAST_BUNDLED_ME_BUDGET_APPLIED.store(UNSET, std::sync::atomic::Ordering::Release);
    if let Ok(mut g) = LAST_BUNDLED_MEDIA_KEY.lock() {
        *g = None;
    }
}

/// Invalidate bundled ME **`vf_smooth_matches_prefs`** so **`apply_mpv_video`** runs **`vf clr`/`vf add`**
/// after **`loadfile`** / **`path`** — a warm mpv+VapourSynth interpreter does not observe a revised
/// ME px² budget unless the **`vf`** is reinstalled (see **`forget`** above).
pub fn forget_bundled_me_budget_vf_apply_on_new_media() {
    forget_bundled_me_budget_vf_apply();
}

pub(crate) fn note_bundled_me_budget_vf_applied(px: u64, media_key: Option<String>) {
    LAST_BUNDLED_ME_BUDGET_APPLIED.store(px, std::sync::atomic::Ordering::Release);
    if let Ok(mut g) = LAST_BUNDLED_MEDIA_KEY.lock() {
        *g = media_key;
    }
}

pub(crate) fn bundled_me_budget_vf_matches_noted_px(
    effective_px: u64,
    v: &crate::db::VideoPrefs,
    media_key: Option<&str>,
) -> bool {
    if !v.vs_path.trim().is_empty() {
        return true;
    }
    let want = effective_px.max(crate::db::MIN_SMOOTH_MAX_AREA);
    if LAST_BUNDLED_ME_BUDGET_APPLIED.load(std::sync::atomic::Ordering::Acquire) != want {
        return false;
    }
    let noted_key = LAST_BUNDLED_MEDIA_KEY.lock().ok().and_then(|g| g.clone());
    noted_key.as_deref() == media_key
}

pub(crate) fn bundled_me_budget_vf_matches_prefs(
    mpv: &libmpv2::Mpv,
    v: &crate::db::VideoPrefs,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> bool {
    let eff = effective_smooth_me_budget_px(mpv, v, bundle);
    let cur_key = me_budget_local_path(mpv, bundle)
        .as_ref()
        .and_then(|p| crate::db::history_key(p.as_path()));
    bundled_me_budget_vf_matches_noted_px(eff, v, cur_key.as_deref())
}

#[cfg(test)]
mod smooth_vf_me_budget_applied_tests {
    use crate::db::VideoPrefs;

    #[test]
    fn bundled_budget_reset_then_mismatch_until_noted() {
        let mut v = VideoPrefs::default();
        v.vs_path.clear();
        v.smooth_max_area = 1_059_297;
        super::forget_bundled_me_budget_vf_apply();
        assert!(
            !super::bundled_me_budget_vf_matches_noted_px(
                v.smooth_max_area.max(crate::db::MIN_SMOOTH_MAX_AREA),
                &v,
                Some("/fake/a.mkv"),
            ),
            "UNSET sentinel must demand a vf rebuild for bundled ME budget"
        );
        let px = v.smooth_max_area.max(crate::db::MIN_SMOOTH_MAX_AREA);
        super::note_bundled_me_budget_vf_applied(px, Some("/fake/a.mkv".into()));
        assert!(
            super::bundled_me_budget_vf_matches_noted_px(px, &v, Some("/fake/a.mkv")),
            "same prefs + noted px² + same media key should satisfy skip-fast-path check"
        );
        assert!(
            !super::bundled_me_budget_vf_matches_noted_px(px, &v, Some("/fake/b.mkv")),
            "different open media must demand vf rebuild even when px² matches"
        );
        v.smooth_max_area = 998_096;
        assert!(
            !super::bundled_me_budget_vf_matches_noted_px(
                v.smooth_max_area.max(crate::db::MIN_SMOOTH_MAX_AREA),
                &v,
                Some("/fake/a.mkv"),
            ),
            "adapted SQLite row must invalidate skip until vf reattach"
        );
    }

    #[test]
    fn custom_vs_path_skips_budget_token() {
        let v = VideoPrefs {
            vs_path: "/tmp/custom.vpy".into(),
            ..Default::default()
        };
        super::forget_bundled_me_budget_vf_apply();
        assert!(super::bundled_me_budget_vf_matches_noted_px(
            v.smooth_max_area.max(crate::db::MIN_SMOOTH_MAX_AREA),
            &v,
            Some("/any/local.mkv"),
        ));
    }
}
