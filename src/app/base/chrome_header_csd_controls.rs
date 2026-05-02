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
    #[cfg(target_os = "macos")]
    sync_header_window_controls_macos(hdr, baseline, show_chrome);
    #[cfg(not(target_os = "macos"))]
    sync_header_window_controls_linux(hdr, baseline, show_chrome);
}

/// macOS: traffic lights live on the NSWindow titlebar, not in the GTK header. Both
/// directions (hide / show) work reliably only when we drive `NSWindow.standardWindowButton`
/// `setHidden:` directly; calling GTK's `set_show_*_title_buttons` here would fight
/// the AppKit state on the next layout pass.
#[cfg(target_os = "macos")]
fn sync_header_window_controls_macos(
    hdr: &adw::HeaderBar,
    _baseline: &Rc<Cell<Option<(bool, bool)>>>,
    show_chrome: bool,
) {
    crate::macos_window::set_traffic_lights_visible(hdr, show_chrome);
}

#[cfg(not(target_os = "macos"))]
fn sync_header_window_controls_linux(
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

    let (s_on, e_on) = baseline
        .get()
        .filter(|&(s, e)| s || e)
        .unwrap_or((true, true));

    if show_chrome {
        hdr.set_show_start_title_buttons(s_on);
        hdr.set_show_end_title_buttons(e_on);
    } else {
        hdr.set_show_start_title_buttons(false);
        hdr.set_show_end_title_buttons(false);
    }
    hdr.queue_allocate();
}
