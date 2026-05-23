/// Rebuild radio rows: **Off** + each sub. Returns **true** if any sub track exists.
///
/// [on_pick] is called with the list label when the user turns **on** a sub track (not **Off**).
/// [on_sub_off] when the user selects **Off** (persist so new files skip fuzzy auto-pick and stay off).
pub fn rebuild_popover(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    block: &Rc<Cell<bool>>,
    gl: &gtk::GLArea,
    on_pick: Option<SubPickFn>,
    on_sub_off: Option<SubOffFn>,
    header_readout: Option<gtk::Label>,
) -> bool {
    while let Some(c) = bx.first_child() {
        bx.remove(&c);
    }
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    let mpv = &b.mpv;
    let rows = sub_rows(mpv);
    if rows.is_empty() {
        return false;
    }
    let off_active = !sub_visibility(mpv);
    let want = current_sid(mpv);
    let fallback = rows.first().map(|r| r.id);
    let hdr_share = Rc::new(header_readout);
    block.set(true);
    let p = Rc::clone(player);
    let gl2 = gl.clone();
    let mut items: Vec<(i64, gtk::CheckButton)> = vec![];

    let off_btn = gtk::CheckButton::with_label("Off");
    let first = off_btn.clone();
    let p_off = Rc::clone(&p);
    let bl_off = Rc::clone(block);
    let g_off = gl2.clone();
    let off_cb = on_sub_off.as_ref().map(Rc::clone);
    let hdr_off = Rc::clone(&hdr_share);
    off_btn.connect_toggled(move |b| {
        if bl_off.get() || !b.is_active() {
            return;
        }
        if let Some(pl) = p_off.borrow().as_ref() {
            set_sub_off(&pl.mpv);
            if let Some(l) = &*hdr_off {
                refresh_sub_header(&pl.mpv, l);
            }
        }
        if let Some(f) = off_cb.as_ref() {
            f();
        }
        g_off.queue_render();
    });
    items.push((-1, off_btn));

    for r in &rows {
        let btn = gtk::CheckButton::with_label(&r.text);
        btn.set_group(Some(&first));
        let id = r.id;
        let label = r.text.clone();
        let p2 = Rc::clone(&p);
        let blk2 = Rc::clone(block);
        let gl3 = gl2.clone();
        let pick = on_pick.as_ref().map(Rc::clone);
        let hdr_pick = Rc::clone(&hdr_share);
        btn.connect_toggled(move |b| {
            if blk2.get() || !b.is_active() {
                return;
            }
            if let Some(pl) = p2.borrow().as_ref() {
                set_sub_id(&pl.mpv, id);
                if let Some(l) = &*hdr_pick {
                    refresh_sub_header(&pl.mpv, l);
                }
            }
            if let Some(f) = pick.as_ref() {
                f(&label);
            }
            gl3.queue_render();
        });
        items.push((r.id, btn));
    }

    for (_, btn) in &items {
        bx.append(btn);
    }
    for (id, btn) in &items {
        if *id == -1 {
            btn.set_active(off_active);
        } else {
            let on = if off_active {
                false
            } else {
                want.or(fallback) == Some(*id)
            };
            btn.set_active(on);
        }
    }
    block.set(false);
    true
}
