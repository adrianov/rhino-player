// Blu-ray / AVCHD shell path (cadence policy: `smooth_prefers_display_resample`).

#[must_use]
pub(crate) fn is_bluray_playback(
    mpv: &libmpv2::Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> bool {
    shell_disc_path(mpv, bundle).is_some()
}

#[must_use]
pub(crate) fn shell_disc_path(
    mpv: &libmpv2::Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> Option<std::path::PathBuf> {
    if mpv
        .get_property::<String>("path")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some_and(|s| mpv_path_is_disc(&s))
    {
        return me_budget_local_path(mpv, bundle);
    }
    me_budget_local_path(mpv, bundle).filter(|p| crate::video_ext::is_bluray_disc_path(p))
}

#[must_use]
pub(crate) fn smooth_prefers_display_resample_bundle(
    mpv: &libmpv2::Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> bool {
    let shell_path = shell_disc_path(mpv, bundle);
    smooth_prefers_display_resample(mpv, shell_path.as_deref())
}
