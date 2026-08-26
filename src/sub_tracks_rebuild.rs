// Subtitles popover assembly: pick handling and top-level rebuild (included from
// `sub_tracks.rs`; row construction lives in `sub_tracks_rebuild_buttons.rs`).

include!("sub_tracks_rebuild_buttons.rs");

/// Everything [apply_sub_pick] needs besides the mpv handle.
struct SubPickRequest<'a> {
    id: i64,
    ifo_slot: Option<u8>,
    label: &'a str,
    shell: Option<&'a std::path::Path>,
    on_pick: Option<&'a SubPickFn>,
    header_readout: Option<&'a gtk::Label>,
    text_color_row: Option<&'a gtk::Box>,
    sub_codecs: Option<&'a [(i64, String)]>,
}

fn apply_sub_pick(mpv: &Mpv, req: &SubPickRequest<'_>) {
    if let Some(sid) = resolve_sub_id(mpv, req.id, req.ifo_slot, req.shell) {
        set_sub_id(mpv, sid);
        save_sub_choice(mpv, sid, req.ifo_slot, req.shell);
    }
    if let Some(f) = req.on_pick {
        f(req.label);
    }
    if let Some(l) = req.header_readout {
        refresh_sub_header(mpv, l, req.shell);
    }
    if let Some(row) = req.text_color_row {
        if let Some(codecs) = req.sub_codecs {
            sync_text_color_row_codecs(mpv, row, codecs);
        } else {
            sync_text_color_row(mpv, row);
        }
    }
    crate::sub_tracks::reapply_styling(mpv);
}

/// Currently active mpv subtitle id plus its DVD IFO slot (when the entity is a DVD).
fn active_sub_want(mpv: &Mpv, shell_ref: Option<&std::path::Path>) -> (Option<i64>, Option<u8>) {
    let want = current_sid(mpv);
    let want_slot = want.and_then(|sid| ifo_slot_for_sid(mpv, sid, shell_ref));
    (want, want_slot)
}

/// **Off** row plus one row per sub track.
fn build_sub_items(
    wiring: &Rc<SubButtonWiring>,
    on_pick: &Option<SubPickFn>,
    on_sub_off: Option<SubOffFn>,
    rows: &[Row],
) -> Vec<(i64, Option<u8>, gtk::CheckButton)> {
    let mut items: Vec<(i64, Option<u8>, gtk::CheckButton)> = Vec::new();
    items.push((-1, None, off_row_button(wiring, on_sub_off)));
    for r in rows {
        let btn = sub_row_button(wiring, on_pick.as_ref().map(Rc::clone), r);
        items.push((r.id, r.ifo_slot, btn));
    }
    items
}

/// Append the freshly built radio rows and reflect the current visibility state.
fn repopulate(
    bx: &gtk::Box,
    block: &Rc<Cell<bool>>,
    wiring: &Rc<SubButtonWiring>,
    mpv: &Mpv,
    on_pick: &Option<SubPickFn>,
    on_sub_off: Option<SubOffFn>,
    rows: &[Row],
) {
    let off_active = !sub_visibility(mpv);
    let (want, want_slot) = active_sub_want(mpv, wiring.shell_path.as_deref());
    block.set(true);
    let items = build_sub_items(wiring, on_pick, on_sub_off, rows);
    group_radio_rows(bx, &items);
    activate_sub_rows(&items, off_active, want, want_slot);
    block.set(false);
}

/// Show the text-styling row only when the active track supports Rhino text styling.
fn sync_color_row(wiring: &SubButtonWiring, mpv: &Mpv) {
    if let Some(row) = wiring.color_row.as_ref() {
        sync_text_color_row_codecs(mpv, row.as_ref(), wiring.codecs_share.as_slice());
    }
}

/// Optional widgets and callbacks for [rebuild_popover].
pub struct SubPopoverParts {
    pub on_pick: Option<SubPickFn>,
    pub on_sub_off: Option<SubOffFn>,
    pub header_readout: Option<gtk::Label>,
    pub text_color_row: Option<gtk::Box>,
}

/// Rebuild radio rows: **Off** + each sub. Returns **true** if any sub track exists.
pub fn rebuild_popover(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    block: &Rc<Cell<bool>>,
    gl: &gtk::GLArea,
    parts: SubPopoverParts,
) -> bool {
    clear_box(bx);
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    let mpv = &b.mpv;
    let shell_path = b.me_budget_shell_path.borrow().clone();
    let (rows, sub_codecs) = sub_popover_data(mpv, shell_path.as_deref());
    if rows.is_empty() {
        return false;
    }
    let wiring = Rc::new(SubButtonWiring::capture(
        player,
        block,
        gl,
        shell_path,
        parts.header_readout,
        parts.text_color_row,
        sub_codecs,
    ));
    repopulate(
        bx,
        block,
        &wiring,
        mpv,
        &parts.on_pick,
        parts.on_sub_off,
        &rows,
    );
    sync_color_row(&wiring, mpv);
    true
}
