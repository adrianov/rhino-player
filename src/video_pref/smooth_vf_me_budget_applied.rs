/// libmpv+VapourSynth often **reuse** an interpreter instance when **`vf vapoursynth:` options are
/// unchanged**. ME budget travels **`user-data=`** digits (script reads **`user_data`** first); env refresh alone cannot retune a warm worker.
/// Rhino records which clamped **`video_smooth_max_area`** px² was last **successfully rebuilt** (`vf clr`/`vf add`)
/// so [vf_smooth_matches_prefs] skips only when SQLite and the bundled script stay in sync.
const UNSET: u64 = u64::MAX;
static LAST_BUNDLED_ME_BUDGET_APPLIED: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(UNSET);

pub(crate) fn forget_bundled_me_budget_vf_apply() {
    LAST_BUNDLED_ME_BUDGET_APPLIED.store(UNSET, std::sync::atomic::Ordering::Release);
}

/// Invalidate bundled ME **`vf_smooth_matches_prefs`** so **`apply_mpv_video`** runs **`vf clr`/`vf add`**
/// after **`loadfile`** / **`path`** — a warm mpv+VapourSynth interpreter does not observe a revised
/// **`video_smooth_max_area`** / **`RHINO_SMOOTH_MAX_AREA`** unless the **`vf`** is reinstalled (see **`forget`** above).
pub fn forget_bundled_me_budget_vf_apply_on_new_media() {
    forget_bundled_me_budget_vf_apply();
}

pub(crate) fn note_bundled_me_budget_vf_applied(px: u64) {
    LAST_BUNDLED_ME_BUDGET_APPLIED.store(px, std::sync::atomic::Ordering::Release);
}

pub(crate) fn bundled_me_budget_vf_matches_noted_px(effective_px: u64, v: &crate::db::VideoPrefs) -> bool {
    if !v.vs_path.trim().is_empty() {
        return true;
    }
    let want = effective_px.max(crate::db::MIN_SMOOTH_MAX_AREA);
    LAST_BUNDLED_ME_BUDGET_APPLIED.load(std::sync::atomic::Ordering::Acquire) == want
}

pub(crate) fn bundled_me_budget_vf_matches_prefs(mpv: &libmpv2::Mpv, v: &crate::db::VideoPrefs) -> bool {
    let eff = effective_smooth_me_budget_px(mpv, v);
    bundled_me_budget_vf_matches_noted_px(eff, v)
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
                &v
            ),
            "UNSET sentinel must demand a vf rebuild for bundled ME budget"
        );
        super::note_bundled_me_budget_vf_applied(v.smooth_max_area.max(crate::db::MIN_SMOOTH_MAX_AREA));
        assert!(
            super::bundled_me_budget_vf_matches_noted_px(
                v.smooth_max_area.max(crate::db::MIN_SMOOTH_MAX_AREA),
                &v
            ),
            "same prefs + noted px² should satisfy skip-fast-path check"
        );
        v.smooth_max_area = 998_096;
        assert!(
            !super::bundled_me_budget_vf_matches_noted_px(
                v.smooth_max_area.max(crate::db::MIN_SMOOTH_MAX_AREA),
                &v
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
            &v
        ));
    }
}
