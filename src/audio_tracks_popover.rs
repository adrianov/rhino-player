// Sound-popover row construction (included from `audio_tracks.rs`; row/label
// matching lives in `audio_tracks_match.rs`).

include!("audio_tracks_match.rs");

fn save_choice(mpv: &Mpv, id: i64, text: &str, shell: Option<&Path>) {
    db::save_audio_track_name(text);
    let Some(path) = media_probe::shell_media_path(mpv, shell) else {
        return;
    };
    let entity = playback_entity::PlaybackEntity::resolve(&path);
    let slot = playback_entity::audio_ifo_slot_for_aid(mpv, &entity, id, shell);
    db::set_audio_track(&entity.db_path(), id, slot);
}

/// Sound popover row pick: set `aid`, persist, and re-align A/V when Smooth presentation is active.
fn apply_user_audio_pick(
    bundle: &MpvBundle,
    row: &AudioMenuRow,
    label: &str,
    shell: Option<&Path>,
) {
    let Some(aid) = resolve_id(&bundle.mpv, row, shell) else {
        return;
    };
    let changed = current_aid(&bundle.mpv) != Some(aid);
    let av_prep = changed
        .then(|| crate::video_pref::snap_audio_track_av_resync(bundle))
        .flatten();
    set_aid(&bundle.mpv, aid);
    save_choice(&bundle.mpv, aid, label, shell);
    if changed {
        crate::video_pref::finish_audio_track_av_resync(bundle, av_prep);
    }
}

fn audio_row_is_active(
    want: Option<i64>,
    want_slot: Option<u8>,
    id: i64,
    ifo_slot: Option<u8>,
) -> bool {
    if want == Some(id) && id > 0 {
        return true;
    }
    matches!((want_slot, ifo_slot), (Some(w), Some(s)) if w == s)
}

fn clear_children(bx: &gtk::Box) {
    while let Some(c) = bx.first_child() {
        bx.remove(&c);
    }
}

/// Apply a user tap on a sound radio row, then refresh the tooltip on the menu button.
fn audio_row_toggled(
    p: &Rc<RefCell<Option<MpvBundle>>>,
    id: i64,
    ifo_slot: Option<u8>,
    label: &str,
    shell_ref: Option<&Path>,
    tip_btn: Option<&gtk::MenuButton>,
) {
    if let Some(pl) = p.borrow().as_ref() {
        let row = AudioMenuRow {
            mpv_id: id,
            label: label.to_string(),
            ifo_slot,
        };
        apply_user_audio_pick(pl, &row, label, shell_ref);
        if let Some(tip_btn) = tip_btn {
            refresh_audio_tooltip(&pl.mpv, tip_btn, shell_ref);
        }
    }
}

/// One radio row wired to [apply_user_audio_pick] plus a tooltip refresh on the menu button.
fn audio_row_button(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    block: &Rc<Cell<bool>>,
    gl: &gtk::GLArea,
    tip_btn: Option<gtk::MenuButton>,
    shell_path: &Option<PathBuf>,
    r: &AudioMenuRow,
) -> gtk::CheckButton {
    let btn = gtk::CheckButton::with_label(&r.label);
    let id = r.mpv_id;
    let ifo_slot = r.ifo_slot;
    let label = r.label.clone();
    let p = Rc::clone(player);
    let blk = Rc::clone(block);
    let gl2 = gl.clone();
    let shell_pick = shell_path.clone();
    btn.connect_toggled(move |b| {
        if blk.get() || !b.is_active() {
            return;
        }
        audio_row_toggled(
            &p,
            id,
            ifo_slot,
            &label,
            shell_pick.as_deref(),
            tip_btn.as_ref(),
        );
        gl2.queue_render();
    });
    btn
}

/// Currently active mpv audio id plus its DVD IFO slot (when the entity is a DVD).
fn active_audio_want(mpv: &Mpv, shell_ref: Option<&Path>) -> (Option<i64>, Option<u8>) {
    let want = current_aid(mpv);
    let want_slot = entity_from_mpv(mpv, shell_ref).and_then(|(entity, _)| {
        want.and_then(|a| audio_ifo_slot_for_aid(mpv, &entity, a, shell_ref))
    });
    (want, want_slot)
}

/// Widget + player handles for one sound-popover rebuild pass.
struct AudioRebuildCtx<'a> {
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    bx: &'a gtk::Box,
    block: &'a Rc<Cell<bool>>,
    gl: &'a gtk::GLArea,
    tooltip_btn: Option<&'a gtk::MenuButton>,
    shell_path: Option<PathBuf>,
}

/// Rebuilds radio rows. Returns **true** if there are **at least two** audio tracks. Clears the
/// box if there is no player or 0–1 track.
pub fn rebuild_popover(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    block: &Rc<Cell<bool>>,
    gl: &gtk::GLArea,
    tooltip_btn: Option<&gtk::MenuButton>,
) -> bool {
    clear_children(bx);
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    let shell_path: Option<PathBuf> = b.me_budget_shell_path.borrow().clone();
    let rows = audio_menu_rows(&b.mpv, shell_path.as_deref());
    if rows.len() < 2 {
        return false;
    }
    let ctx = AudioRebuildCtx {
        player,
        bx,
        block,
        gl,
        tooltip_btn,
        shell_path,
    };
    repopulate(&ctx, &b.mpv, &rows);
    true
}

/// Build, group, append, and mark the radio rows for the current audio state.
fn repopulate(ctx: &AudioRebuildCtx, mpv: &Mpv, rows: &[AudioMenuRow]) {
    let (want, want_slot) = active_audio_want(mpv, ctx.shell_path.as_deref());
    ctx.block.set(true);
    let mut buttons: Vec<(i64, Option<u8>, gtk::CheckButton)> = Vec::new();
    for r in rows {
        let btn = audio_row_button(
            ctx.player,
            ctx.block,
            ctx.gl,
            ctx.tooltip_btn.cloned(),
            &ctx.shell_path,
            r,
        );
        buttons.push((r.mpv_id, r.ifo_slot, btn));
    }
    group_radio_rows(ctx.bx, &buttons);
    for (id, ifo_slot, btn) in &buttons {
        btn.set_active(audio_row_is_active(want, want_slot, *id, *ifo_slot));
    }
    ctx.block.set(false);
}
/// Make the first row the radio-group leader, then append every row to the box.
fn group_radio_rows(bx: &gtk::Box, buttons: &[(i64, Option<u8>, gtk::CheckButton)]) {
    if let Some((_, _, first)) = buttons.first() {
        for (_, _, later) in buttons.iter().skip(1) {
            later.set_group(Some(first));
        }
    }
    for (_, _, btn) in buttons {
        bx.append(btn);
    }
}
