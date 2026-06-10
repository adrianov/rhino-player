fn persist_budget_current_effective(
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
) -> Option<u64> {
    let g = player.try_borrow().ok()?;
    let b = g.as_ref()?;
    let vp = video_pref.borrow();
    Some(effective_smooth_me_budget_px(&b.mpv, &vp, Some(b)))
}

fn persist_budget_save_media_row_verbose(
    b: &crate::mpv_embed::MpvBundle,
    new_budget_px: u64,
    stderr_reason_suffix: &'static str,
) {
    if let Some(p) = crate::media_probe::shell_media_path(
        &b.mpv,
        b.me_budget_shell_path.borrow().as_deref(),
    ) {
        let entity = crate::playback_entity::db_path_for(&p);
        crate::db::media_save_smooth_me_budget(&entity, new_budget_px);
        if video_log() {
            let key_ok = crate::db::history_key(&entity).is_some();
            eprintln!(
                "[rhino] video: (verbose) persist_budget media_save px²={new_budget_px} history_key_ok={key_ok}",
            );
        }
    } else if video_log() {
        eprintln!(
            "[rhino] video: (verbose) persist_budget no local path for media_save {stderr_reason_suffix}",
        );
    }
}

fn persist_budget_save_media_imm_borrow(
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    new_budget_px: u64,
) {
    if let Ok(g) = player.try_borrow() {
        if let Some(b) = g.as_ref() {
            if let Some(p) = crate::media_probe::shell_media_path(
                &b.mpv,
                b.me_budget_shell_path.borrow().as_deref(),
            ) {
                crate::db::media_save_smooth_me_budget(
                    &crate::playback_entity::db_path_for(&p),
                    new_budget_px,
                );
            }
        }
    }
}

/// Persist a new bundled **ME px²** for the **open `media` row** and run **`apply_mpv_video`** (**`RHINO_SMOOTH_MAX_AREA`**,
/// **`vf`**). Does **not** change **`VideoPrefs.smooth_max_area`** (Preferences default / neighbor fallback) — adaptive
/// overload and recovery are **per-file** on **`media.smooth_me_budget_px2`** so a strain shrink on one clip does not
/// become the cap for the next **`loadfile`**.
#[must_use]
fn persist_budget_and_maybe_rebuild_vf(
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
    new_budget_px: u64,
    stderr_reason_suffix: &'static str,
) -> bool {
    if player
        .try_borrow()
        .ok()
        .and_then(|g| g.as_ref().map(|b| b.may_persist_media_rows()))
        != Some(true)
    {
        return false;
    }
    let Some(eff) = persist_budget_current_effective(player, video_pref) else {
        if video_log() {
            eprintln!(
                "[rhino] video: (verbose) persist_budget skip (no player) {stderr_reason_suffix}"
            );
        }
        return false;
    };
    if new_budget_px == eff {
        if video_log() {
            eprintln!(
                "[rhino] smooth: persist_skip ME_budget_px² already {eff} ({stderr_reason_suffix})",
            );
        }
        return false;
    }

    forget_bundled_me_budget_vf_apply();
    match player.try_borrow_mut() {
        Ok(mut g) => {
            let Some(b) = g.as_mut() else {
                return true;
            };
            // Same `borrow_mut` as `apply_mpv_video`: a failing `try_borrow()` before `try_borrow_mut()`
            // used to skip `media_save_smooth_me_budget` while vf still rebuilt — then `resolve_media_smooth_me_budget`
            // fell through to another file's lower neighbor px² and **`smooth_cap`** lagged prefs.
            persist_budget_save_media_row_verbose(b, new_budget_px, stderr_reason_suffix);
            // Release the player borrow before reapplying: `apply_mpv_video` borrows `player` itself.
            drop(g);
            let mut vp = video_pref.borrow_mut();
            let _ = apply_mpv_video(player, &mut vp, None);
        }
        Err(_) => {
            if video_log() {
                eprintln!(
                    "[rhino] video: (verbose) persist_budget px²={new_budget_px} could_not_borrow_player_mut_for_vf_apply (best-effort media_save via imm borrow)",
                );
            }
            persist_budget_save_media_imm_borrow(player, new_budget_px);
        }
    }
    true
}
