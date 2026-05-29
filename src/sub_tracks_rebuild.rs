/// Rebuild radio rows: **Off** + each sub. Returns **true** if any sub track exists.
///
/// [on_pick] is called with the list label when the user turns **on** a sub track (not **Off**).
/// [on_sub_off] when the user selects **Off** (persist so new files skip fuzzy auto-pick and stay off).
fn sub_row_is_active(
    off_active: bool,
    want: Option<i64>,
    want_slot: Option<u8>,
    id: i64,
    ifo_slot: Option<u8>,
) -> bool {
    if off_active {
        return false;
    }
    if want == Some(id) && id > 0 {
        return true;
    }
    matches!((want_slot, ifo_slot), (Some(w), Some(s)) if w == s)
}

fn apply_sub_pick(
    mpv: &Mpv,
    id: i64,
    ifo_slot: Option<u8>,
    label: &str,
    shell: Option<&std::path::Path>,
    on_pick: Option<&SubPickFn>,
    header_readout: Option<&gtk::Label>,
    text_color_row: Option<&gtk::Box>,
    sub_codecs: Option<&[(i64, String)]>,
) {
    if let Some(sid) = resolve_sub_id(mpv, id, ifo_slot, shell) {
        set_sub_id(mpv, sid);
        save_sub_choice(mpv, sid, ifo_slot, shell);
    }
    if let Some(f) = on_pick {
        f(label);
    }
    if let Some(l) = header_readout {
        refresh_sub_header(mpv, l, shell);
    }
    if let Some(row) = text_color_row {
        if let Some(codecs) = sub_codecs {
            sync_text_color_row_codecs(mpv, row, codecs);
        } else {
            sync_text_color_row(mpv, row);
        }
    }
    crate::sub_tracks::reapply_styling(mpv);
}

pub fn rebuild_popover(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    block: &Rc<Cell<bool>>,
    gl: &gtk::GLArea,
    on_pick: Option<SubPickFn>,
    on_sub_off: Option<SubOffFn>,
    header_readout: Option<gtk::Label>,
    text_color_row: Option<gtk::Box>,
) -> bool {
    while let Some(c) = bx.first_child() {
        bx.remove(&c);
    }
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    let mpv = &b.mpv;
    let shell_path = b.me_budget_shell_path.borrow().clone();
    let shell_ref = shell_path.as_deref();
    let (rows, sub_codecs) = sub_popover_data(mpv, shell_ref);
    if rows.is_empty() {
        return false;
    }
    let off_active = !sub_visibility(mpv);
    let want = current_sid(mpv);
    let want_slot = want.and_then(|sid| ifo_slot_for_sid(mpv, sid, shell_ref));
    let hdr_share = Rc::new(header_readout);
    let color_row = text_color_row.map(Rc::new);
    let codecs_share = Rc::new(sub_codecs);
    block.set(true);
    let p = Rc::clone(player);
    let gl2 = gl.clone();
    let mut items: Vec<(i64, Option<u8>, gtk::CheckButton)> = vec![];

    let off_btn = gtk::CheckButton::with_label("Off");
    let first = off_btn.clone();
    let p_off = Rc::clone(&p);
    let bl_off = Rc::clone(block);
    let g_off = gl2.clone();
    let off_cb = on_sub_off.as_ref().map(Rc::clone);
    let hdr_off = Rc::clone(&hdr_share);
    let color_off = color_row.clone();
    let codecs_off = Rc::clone(&codecs_share);
    let shell_off = shell_path.clone();
    off_btn.connect_toggled(move |b| {
        if bl_off.get() || !b.is_active() {
            return;
        }
        if let Some(pl) = p_off.borrow().as_ref() {
            let shell_ref = shell_off.as_deref();
            set_sub_off(&pl.mpv);
            if let Some(l) = &*hdr_off {
                refresh_sub_header(&pl.mpv, l, shell_ref);
            }
            if let Some(row) = color_off.as_ref() {
                sync_text_color_row_codecs(&pl.mpv, row.as_ref(), &codecs_off);
            }
        }
        if let Some(f) = off_cb.as_ref() {
            f();
        }
        g_off.queue_render();
    });
    items.push((-1, None, off_btn));

    for r in &rows {
        let btn = gtk::CheckButton::with_label(&r.text);
        btn.set_group(Some(&first));
        let id = r.id;
        let ifo_slot = r.ifo_slot;
        let label = r.text.clone();
        let p2 = Rc::clone(&p);
        let blk2 = Rc::clone(block);
        let gl3 = gl2.clone();
        let pick = on_pick.as_ref().map(Rc::clone);
        let hdr_pick = Rc::clone(&hdr_share);
        let color_pick = color_row.clone();
        let codecs_pick = Rc::clone(&codecs_share);
        let shell_pick = shell_path.clone();
        btn.connect_toggled(move |b| {
            if blk2.get() || !b.is_active() {
                return;
            }
            if let Some(pl) = p2.borrow().as_ref() {
                let shell_ref = shell_pick.as_deref();
                apply_sub_pick(
                    &pl.mpv,
                    id,
                    ifo_slot,
                    &label,
                    shell_ref,
                    pick.as_ref(),
                    hdr_pick.as_ref().as_ref(),
                    color_pick.as_deref(),
                    Some(codecs_pick.as_slice()),
                );
            }
            gl3.queue_render();
        });
        items.push((r.id, ifo_slot, btn));
    }

    for (_, _, btn) in &items {
        bx.append(btn);
    }
    for (id, ifo_slot, btn) in &items {
        if *id == -1 {
            btn.set_active(off_active);
        } else {
            btn.set_active(sub_row_is_active(
                off_active, want, want_slot, *id, *ifo_slot,
            ));
        }
    }
    block.set(false);
    if let Some(row) = color_row.as_ref() {
        sync_text_color_row_codecs(mpv, row.as_ref(), codecs_share.as_slice());
    }
    true
}
