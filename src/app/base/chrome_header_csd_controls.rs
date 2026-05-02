fn wire_header_csd_baseline_snap(
    baseline: &Rc<Cell<Option<(bool, bool)>>>,
    header: &adw::HeaderBar,
) {
    let bc = Rc::clone(baseline);
    let h_snap = header.clone();
    header.connect_map(move |_hb| {
        if bc.get().is_some() {
            return;
        }
        let bc2 = Rc::clone(&bc);
        let h2 = h_snap.clone();
        glib::idle_add_local_once(move || {
            if bc2.get().is_some() {
                return;
            }
            let s = h2.shows_start_title_buttons();
            let e = h2.shows_end_title_buttons();
            if s || e {
                bc2.set(Some((s, e)));
            }
        });
    });
}

fn sync_header_window_controls(
    hdr: &adw::HeaderBar,
    baseline: &Rc<Cell<Option<(bool, bool)>>>,
    show_chrome: bool,
) {
    // Never snapshot `(false,false)`: hiding runs before a naive "first mapped read" captures only
    // false forever. Capture only while chrome is shown and GTK reports a decorated side lit.
    if show_chrome && baseline.get().is_none() && hdr.is_mapped() {
        let s = hdr.shows_start_title_buttons();
        let e = hdr.shows_end_title_buttons();
        if s || e {
            baseline.set(Some((s, e)));
        }
    }

    let restore = baseline.get().filter(|&(s, e)| s || e);

    if show_chrome {
        match restore {
            Some((s0, e0)) => {
                hdr.set_show_start_title_buttons(s0);
                hdr.set_show_end_title_buttons(e0);
            }
            None => {
                hdr.set_show_start_title_buttons(true);
                hdr.set_show_end_title_buttons(true);
            }
        }
        hdr.queue_allocate();
    } else {
        hdr.set_show_start_title_buttons(false);
        hdr.set_show_end_title_buttons(false);
        hdr.queue_allocate();
    }
}
