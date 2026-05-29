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

fn sub_pop_show_tracks_impl(
    p: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    blk: &Rc<Cell<bool>>,
    gla: &gtk::GLArea,
    sec: &gtk::Box,
    on_pick: Option<Rc<dyn Fn(&str)>>,
    on_sub_off: Option<Rc<dyn Fn()>>,
    header_readout: Option<gtk::Label>,
    text_color_row: Option<gtk::Box>,
) {
    let show = sub_tracks::rebuild_popover(
        p,
        bx,
        blk,
        gla,
        on_pick,
        on_sub_off,
        header_readout,
        text_color_row,
    );
    sec.set_visible(show);
}

fn sub_pop_show_tracks(
    p: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    blk: &Rc<Cell<bool>>,
    gla: &gtk::GLArea,
    sec: &gtk::Box,
    on_pick: Option<Rc<dyn Fn(&str)>>,
    on_sub_off: Option<Rc<dyn Fn()>>,
    header_readout: Option<gtk::Label>,
    text_color_row: Option<gtk::Box>,
) {
    sub_pop_show_tracks_impl(
        p,
        bx,
        blk,
        gla,
        sec,
        on_pick,
        on_sub_off,
        header_readout,
        text_color_row,
    );
}

fn wire_popover_shows(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    w: &WindowWidgets,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
) {
    let (p, bx, blk, gla, sec, vol_menu) = (
        player.clone(),
        w.audio_tracks_box.clone(),
        Rc::clone(&w.audio_tracks_block),
        w.gl_area.clone(),
        w.audio_tracks_section.clone(),
        w.vol_menu.clone(),
    );
    let audio_open = {
        let (p, bx, blk, gla, sec, vol_menu) =
            (p.clone(), bx.clone(), blk.clone(), gla.clone(), sec.clone(), vol_menu.clone());
        Rc::new(move || vol_pop_show_tracks(&p, &bx, &blk, &gla, &sec, &vol_menu))
    };
    w.vol_pop.connect_show({
        let audio_open = Rc::clone(&audio_open);
        move |_| audio_open()
    });

    let sp_pick = sub_pref.clone();
    let sp_off = sub_pref.clone();
    let on_sub_pick: Rc<dyn Fn(&str)> = Rc::new(move |label: &str| {
        let mut s = sp_pick.borrow_mut();
        s.last_sub_label = label.to_string();
        s.sub_off = false;
        db::save_sub(&s);
    });
    let on_sub_off: Rc<dyn Fn()> = Rc::new(move || {
        sp_off.borrow_mut().sub_off = true;
        db::save_sub(&sp_off.borrow());
    });
    let (p2, bx2, blk2, gla2, sec2) = (
        player.clone(),
        w.sub_tracks_box.clone(),
        Rc::clone(&w.sub_tracks_block),
        w.gl_area.clone(),
        w.sub_tracks_section.clone(),
    );
    let sub_rd = w.sub_readout.clone();
    let sub_color_row = w.sub_color_row.clone();
    let sub_pick = Rc::clone(&on_sub_pick);
    let sub_off = Rc::clone(&on_sub_off);
    let sub_open = {
        let p2 = p2.clone();
        let bx2 = bx2.clone();
        let blk2 = Rc::clone(&blk2);
        let gla2 = gla2.clone();
        let sec2 = sec2.clone();
        let sub_pick = Rc::clone(&sub_pick);
        let sub_off = Rc::clone(&sub_off);
        let sub_rd = sub_rd.clone();
        let sub_color_row = sub_color_row.clone();
        Rc::new(move || {
            sub_pop_show_tracks(
                &p2,
                &bx2,
                &blk2,
                &gla2,
                &sec2,
                Some(Rc::clone(&sub_pick)),
                Some(Rc::clone(&sub_off)),
                Some(sub_rd.clone()),
                Some(sub_color_row.clone()),
            );
        })
    };
    w.sub_pop.connect_show({
        let sub_open = Rc::clone(&sub_open);
        move |_| sub_open()
    });

    crate::header_menu_tracks::register_refresh(crate::header_menu_tracks::HeaderMenuTrackHooks {
        audio: audio_open,
        sub: sub_open,
    });
}
