fn vol_pop_show_tracks(
    p: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    blk: &Rc<Cell<bool>>,
    gla: &gtk::GLArea,
    sec: &gtk::Box,
) {
    let (p, bx, blk, gla, sec) = (p.clone(), bx.clone(), blk.clone(), gla.clone(), sec.clone());
    let _ = glib::idle_add_local_once(move || {
        let show = audio_tracks::rebuild_popover(&p, &bx, &blk, &gla);
        sec.set_visible(show);
    });
}

fn wire_popover_shows(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    w: &WindowWidgets,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
) {
    let (p, bx, blk, gla, sec) = (
        player.clone(),
        w.audio_tracks_box.clone(),
        Rc::clone(&w.audio_tracks_block),
        w.gl_area.clone(),
        w.audio_tracks_section.clone(),
    );
    w.vol_pop.connect_show(move |_| {
        vol_pop_show_tracks(&p, &bx, &blk, &gla, &sec);
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
    let sub_pick = Rc::clone(&on_sub_pick);
    let sub_off = Rc::clone(&on_sub_off);
    w.sub_pop.connect_show(move |_| {
        let (p2, bx2, blk2, gla2, sec2) = (
            p2.clone(),
            bx2.clone(),
            blk2.clone(),
            gla2.clone(),
            sec2.clone(),
        );
        let sub_pick = sub_pick.clone();
        let sub_off = sub_off.clone();
        let sub_rd = sub_rd.clone();
        let _ = glib::idle_add_local_once(move || {
            let show = sub_tracks::rebuild_popover(
                &p2,
                &bx2,
                &blk2,
                &gla2,
                Some(sub_pick),
                Some(sub_off),
                Some(sub_rd),
            );
            sec2.set_visible(show);
        });
    });
}
