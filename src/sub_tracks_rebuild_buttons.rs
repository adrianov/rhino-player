// Radio-row construction for the subtitles popover (included from `sub_tracks_rebuild.rs`).

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

/// Shared per-button state, cheaply clonable into each toggled callback.
struct SubButtonWiring {
    player: Rc<RefCell<Option<MpvBundle>>>,
    block: Rc<Cell<bool>>,
    gl: gtk::GLArea,
    shell_path: Option<std::path::PathBuf>,
    hdr_share: Rc<Option<gtk::Label>>,
    color_row: Option<Rc<gtk::Box>>,
    codecs_share: Rc<Vec<(i64, String)>>,
}

impl SubButtonWiring {
    /// Snapshot the shared per-button state for one popover rebuild.
    fn capture(
        player: &Rc<RefCell<Option<MpvBundle>>>,
        block: &Rc<Cell<bool>>,
        gl: &gtk::GLArea,
        shell_path: Option<std::path::PathBuf>,
        header_readout: Option<gtk::Label>,
        text_color_row: Option<gtk::Box>,
        codecs: Vec<(i64, String)>,
    ) -> Self {
        Self {
            player: Rc::clone(player),
            block: Rc::clone(block),
            gl: gl.clone(),
            shell_path,
            hdr_share: Rc::new(header_readout),
            color_row: text_color_row.map(Rc::new),
            codecs_share: Rc::new(codecs),
        }
    }
}

fn clear_box(bx: &gtk::Box) {
    while let Some(c) = bx.first_child() {
        bx.remove(&c);
    }
}

/// Handle a user tap on **Off**: hide subs, refresh readouts, persist the off choice.
fn off_row_toggled(w: &SubButtonWiring, on_sub_off: Option<&SubOffFn>) {
    if let Some(pl) = w.player.borrow().as_ref() {
        let shell_ref = w.shell_path.as_deref();
        set_sub_off(&pl.mpv);
        if let Some(l) = w.hdr_share.as_ref() {
            refresh_sub_header(&pl.mpv, l, shell_ref);
        }
        if let Some(row) = w.color_row.as_ref() {
            sync_text_color_row_codecs(&pl.mpv, row.as_ref(), &w.codecs_share);
        }
    }
    if let Some(f) = on_sub_off {
        f();
    }
}

/// The **Off** radio row.
fn off_row_button(wiring: &Rc<SubButtonWiring>, on_sub_off: Option<SubOffFn>) -> gtk::CheckButton {
    let btn = gtk::CheckButton::with_label("Off");
    let w = Rc::clone(wiring);
    btn.connect_toggled(move |b| {
        if w.block.get() || !b.is_active() {
            return;
        }
        off_row_toggled(&w, on_sub_off.as_ref());
        w.gl.queue_render();
    });
    btn
}

/// Apply a user tap on a subtitle radio row.
fn sub_row_toggled(
    w: &SubButtonWiring,
    id: i64,
    ifo_slot: Option<u8>,
    label: &str,
    pick: Option<&SubPickFn>,
) {
    if let Some(pl) = w.player.borrow().as_ref() {
        apply_sub_pick(
            &pl.mpv,
            &SubPickRequest {
                id,
                ifo_slot,
                label,
                shell: w.shell_path.as_deref(),
                on_pick: pick,
                header_readout: w.hdr_share.as_ref().as_ref(),
                text_color_row: w.color_row.as_deref(),
                sub_codecs: Some(w.codecs_share.as_slice()),
            },
        );
    }
}

/// One subtitle radio row wired to [apply_sub_pick].
fn sub_row_button(
    wiring: &Rc<SubButtonWiring>,
    on_pick: Option<SubPickFn>,
    r: &Row,
) -> gtk::CheckButton {
    let btn = gtk::CheckButton::with_label(&r.text);
    let id = r.id;
    let ifo_slot = r.ifo_slot;
    let label = r.text.clone();
    let pick = on_pick;
    let w = Rc::clone(wiring);
    btn.connect_toggled(move |b| {
        if w.block.get() || !b.is_active() {
            return;
        }
        sub_row_toggled(&w, id, ifo_slot, &label, pick.as_ref());
        w.gl.queue_render();
    });
    btn
}

/// Make the first row the radio-group leader (**Off**), then append every row to the box.
fn group_radio_rows(bx: &gtk::Box, items: &[(i64, Option<u8>, gtk::CheckButton)]) {
    if let Some((_, _, first)) = items.first() {
        for (_, _, later) in items.iter().skip(1) {
            later.set_group(Some(first));
        }
    }
    for (_, _, btn) in items {
        bx.append(btn);
    }
}

/// Reflect current visibility: **Off** checked when subs are hidden, else the active track.
fn activate_sub_rows(
    items: &[(i64, Option<u8>, gtk::CheckButton)],
    off_active: bool,
    want: Option<i64>,
    want_slot: Option<u8>,
) {
    for (id, ifo_slot, btn) in items {
        if *id == -1 {
            btn.set_active(off_active);
        } else {
            btn.set_active(sub_row_is_active(
                off_active, want, want_slot, *id, *ifo_slot,
            ));
        }
    }
}
