fn vol_pop_show_tracks_impl(
    p: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    blk: &Rc<Cell<bool>>,
    gla: &gtk::GLArea,
    sec: &gtk::Box,
    vol_menu: &gtk::MenuButton,
) {
    let show = audio_tracks::rebuild_popover(p, bx, blk, gla, Some(vol_menu));
    audio_tracks::refresh_audio_tooltip_for_player(p, vol_menu);
    sec.set_visible(show);
}

fn vol_pop_show_tracks(
    p: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    blk: &Rc<Cell<bool>>,
    gla: &gtk::GLArea,
    sec: &gtk::Box,
    vol_menu: &gtk::MenuButton,
) {
    vol_pop_show_tracks_impl(p, bx, blk, gla, sec, vol_menu);
}

/// Shared refs to a track-list popover's surfaces (player, list box, block, gl area, section).
struct TrackPopRefs<'a> {
    p: &'a Rc<RefCell<Option<MpvBundle>>>,
    bx: &'a gtk::Box,
    blk: &'a Rc<Cell<bool>>,
    gla: &'a gtk::GLArea,
    sec: &'a gtk::Box,
}

fn sub_pop_show_tracks_impl(t: TrackPopRefs<'_>, parts: sub_tracks::SubPopoverParts) {
    let show = sub_tracks::rebuild_popover(t.p, t.bx, t.blk, t.gla, parts);
    t.sec.set_visible(show);
}

fn sub_pop_show_tracks(
    t: TrackPopRefs<'_>,
    on_pick: Option<SubPickHook>,
    on_sub_off: Option<PopShowHook>,
    header_readout: Option<gtk::Label>,
    text_color_row: Option<gtk::Box>,
) {
    sub_pop_show_tracks_impl(
        t,
        sub_tracks::SubPopoverParts {
            on_pick,
            on_sub_off,
            header_readout,
            text_color_row,
        },
    );
}

fn wire_popover_shows(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    w: &WindowWidgets,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
) {
    let audio_open = make_audio_show_hook(player, w);
    w.vol_pop.connect_show({
        let audio_open = Rc::clone(&audio_open);
        move |_| audio_open()
    });

    let sub_open = make_sub_show_hook(player, w, sub_pref);
    w.sub_pop.connect_show({
        let sub_open = Rc::clone(&sub_open);
        move |_| sub_open()
    });

    crate::header_menu_tracks::register_refresh(crate::header_menu_tracks::HeaderMenuTrackHooks {
        audio: audio_open,
        sub: sub_open,
    });
}

type PopShowHook = Rc<dyn Fn()>;

type SubPickHook = Rc<dyn Fn(&str)>;

type SubPrefHooks = (SubPickHook, PopShowHook);

fn make_audio_show_hook(player: &Rc<RefCell<Option<MpvBundle>>>, w: &WindowWidgets) -> PopShowHook {
    let (p, bx, blk, gla, sec, vol_menu) = (
        player.clone(),
        w.audio_tracks_box.clone(),
        Rc::clone(&w.audio_tracks_block),
        w.gl_area.clone(),
        w.audio_tracks_section.clone(),
        w.vol_menu.clone(),
    );
    Rc::new(move || vol_pop_show_tracks(&p, &bx, &blk, &gla, &sec, &vol_menu))
}

fn make_sub_show_hook(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    w: &WindowWidgets,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
) -> PopShowHook {
    let (sub_pick, sub_off) = make_sub_pref_hooks(sub_pref);
    let p = player.clone();
    let bx = w.sub_tracks_box.clone();
    let blk = Rc::clone(&w.sub_tracks_block);
    let gla = w.gl_area.clone();
    let sec = w.sub_tracks_section.clone();
    let rd = w.sub_readout.clone();
    let color_row = w.sub_color_row.clone();
    Rc::new(move || {
        open_sub_tracks(
            TrackPopRefs {
                p: &p,
                bx: &bx,
                blk: &blk,
                gla: &gla,
                sec: &sec,
            },
            &sub_pick,
            &sub_off,
            &rd,
            &color_row,
        )
    })
}

/// Preference writes for a subtitle pick / subtitle-off toggle.
fn make_sub_pref_hooks(sub_pref: &Rc<RefCell<db::SubPrefs>>) -> SubPrefHooks {
    let sp_pick = sub_pref.clone();
    let sp_off = sub_pref.clone();
    let on_pick: SubPickHook = Rc::new(move |label: &str| {
        let mut s = sp_pick.borrow_mut();
        s.last_sub_label = label.to_string();
        s.sub_off = false;
        db::save_sub(&s);
    });
    let on_off: PopShowHook = Rc::new(move || {
        sp_off.borrow_mut().sub_off = true;
        db::save_sub(&sp_off.borrow());
    });
    (on_pick, on_off)
}

fn open_sub_tracks(
    t: TrackPopRefs<'_>,
    pick: &SubPickHook,
    off: &PopShowHook,
    rd: &gtk::Label,
    color_row: &gtk::Box,
) {
    sub_pop_show_tracks(
        t,
        Some(Rc::clone(pick)),
        Some(Rc::clone(off)),
        Some(rd.clone()),
        Some(color_row.clone()),
    );
}
