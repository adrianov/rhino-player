/// Builds the playback-speed popover + compact readout directly under the icon and wires handlers.
/// Returns the list box (needed for file-loaded sync) + sync flag.
struct SpeedMenuResult {
    speed_readout: gtk::Label,
    speed_mbtn: gtk::MenuButton,
    speed_list: gtk::ListBox,
    speed_sync: Rc<Cell<bool>>,
}

fn build_speed_menu(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl: &gtk::GLArea,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    app: &adw::Application,
) -> SpeedMenuResult {
    let speed_list = gtk::ListBox::new();
    speed_list.set_activate_on_single_click(true);
    speed_list.add_css_class("rich-list");
    for s in &playback_speed::SPEEDS {
        let row = gtk::ListBoxRow::new();
        let lab = gtk::Label::new(Some(&playback_speed::format_step(*s)));
        lab.set_halign(gtk::Align::Start);
        lab.set_margin_start(10);
        lab.set_margin_end(10);
        lab.set_margin_top(6);
        lab.set_margin_bottom(6);
        row.set_child(Some(&lab));
        speed_list.append(&row);
    }
    let speed_col = gtk::Box::new(gtk::Orientation::Vertical, 6);
    speed_col.add_css_class("rp-popover-box");
    speed_col.append(&speed_list);
    let speed_pop = gtk::Popover::new();
    speed_pop.add_css_class("rp-header-popover");
    speed_pop.set_child(Some(&speed_col));
    header_popover_non_modal(&speed_pop);
    let speed_mbtn = gtk::MenuButton::new();
    speed_mbtn.set_icon_name("speedometer-symbolic");
    speed_mbtn.set_tooltip_text(Some("Playback speed"));
    speed_mbtn.set_popover(Some(&speed_pop));
    speed_mbtn.set_sensitive(false);
    speed_mbtn.add_css_class("flat");
    speed_mbtn.set_halign(gtk::Align::Center);
    speed_mbtn.set_hexpand(false);

    let speed_readout = gtk::Label::new(Some(&playback_speed::format_step(1.0)));
    speed_readout.add_css_class("rp-speed-readout");
    speed_readout.set_halign(gtk::Align::Center);
    speed_readout.set_xalign(0.5);
    speed_readout.set_sensitive(false);

    let speed_sync = Rc::new(Cell::new(false));
    {
        let p = player.clone();
        let glr = gl.clone();
        let sy = speed_sync.clone();
        let smb = speed_mbtn.clone();
        let spd_lbl = speed_readout.clone();
        let vp = Rc::clone(video_pref);
        let ap = app.clone();
        speed_list.connect_row_activated(move |list2, row| {
            if sy.get() {
                return;
            }
            let i: u32 = (0i32..playback_speed::SPEEDS.len() as i32)
                .find(|&ix| list2.row_at_index(ix).is_some_and(|r| r == *row))
                .unwrap_or(0) as u32;
            let v = playback_speed::value_at(i);
            if let Some(b) = p.borrow().as_ref() {
                let _ = b.mpv.set_property("speed", v);
                playback_speed::stamp_speed_readout(&spd_lbl, v);
                glr.queue_render();
            }
            // Defer vf rebuild: libmpv can still report the old speed on the same GTK tick as
            // set_property; mvtools_vf_eligible + add_smooth_60 must see 1.0× when returning.
            let bref = p.clone();
            let vp2 = Rc::clone(&vp);
            let ap2 = ap.clone();
            let _ = glib::idle_add_local_once(move || {
                let Some(ref pl) = *bref.borrow() else { return };
                let r = video_pref::refresh_smooth_for_playback_speed(pl, &mut vp2.borrow_mut(), Some(v));
                if r.smooth_auto_off {
                    sync_smooth_60_to_off(&ap2);
                }
            });
            smb.set_active(false);
        });
    }
    SpeedMenuResult {
        speed_readout,
        speed_mbtn,
        speed_list,
        speed_sync,
    }
}
