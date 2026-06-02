/// Pause → attach VapourSynth vf → apply present opts → seek back → optional unpause.
/// Returns false if attach failed (prefs already turned off).
pub(crate) fn attach_vf_paused(
    mpv: &Mpv,
    v: &mut VideoPrefs,
    p_esc: &str,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    me_cap: u64,
    restore_play: bool,
) -> bool {
    let was_playing = !mpv.get_property::<bool>("pause").unwrap_or(true);
    if was_playing {
        let _ = mpv.set_property("pause", true);
    }
    if !smooth_vapoursynth_vf_try_attach(mpv, p_esc, bundle) {
        turn_off_smooth_60_in_prefs(v);
        if was_playing && restore_play {
            let _ = mpv.set_property("pause", false);
        }
        return false;
    }
    if v.vs_path.trim().is_empty() {
        let media_key = me_budget_local_path(mpv, bundle)
            .as_ref()
            .and_then(|p| crate::db::history_key(p.as_path()));
        note_bundled_me_budget_vf_applied(me_cap, media_key);
    }
    set_present_opts(mpv, true);
    // No position seek: mpv keeps A/V aligned by PTS through a live `vf add` (`avsync` ~0). A seek
    // here only moved the picture (visible jump on toggle) without changing sync. Resume / scrub
    // own real position changes; here we just unpause and refresh the render so the new graph shows.
    let state = if was_playing { "playing" } else { "paused" };
    eprintln!("[rhino] video: smooth-on vf attached ({state}, no seek)");
    if was_playing && restore_play {
        let _ = mpv.set_property("pause", false);
    }
    vf_av_ping_render(bundle);
    true
}
