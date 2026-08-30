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
    root: &adw::ToolbarView,
) {
    #[cfg(target_os = "macos")]
    {
        let _ = (baseline, root);
        sync_header_window_controls_macos(hdr, show_chrome);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = root;
        sync_header_window_controls_linux(hdr, baseline, show_chrome);
    }
}

/// macOS: show/hide native stoplights with chrome (layout owned by `macos_traffic_vertical`).
#[cfg(target_os = "macos")]
fn sync_header_window_controls_macos(hdr: &adw::HeaderBar, show_chrome: bool) {
    use gtk::prelude::WidgetExt;

    if crate::macos_fs_exit::exit_armed() {
        // Do not touch traffic lights while exiting — AppKit titlebar layout is fragile.
        return;
    }

    let fullscreen = hdr
        .root()
        .and_then(|w| w.downcast::<adw::ApplicationWindow>().ok())
        .is_some_and(|win| win.is_fullscreen());

    crate::macos_window::set_traffic_lights_visible(hdr, fullscreen || show_chrome);
}

#[cfg(not(target_os = "macos"))]
fn capture_linux_baseline(
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
}

/// Apply the captured baseline (or GTK defaults) to both title-button sides.
#[cfg(not(target_os = "macos"))]
fn apply_linux_title_buttons(hdr: &adw::HeaderBar, s_on: bool, e_on: bool, show_chrome: bool) {
    if show_chrome {
        hdr.set_show_start_title_buttons(s_on);
        hdr.set_show_end_title_buttons(e_on);
    } else {
        hdr.set_show_start_title_buttons(false);
        hdr.set_show_end_title_buttons(false);
    }
}

#[cfg(not(target_os = "macos"))]
fn sync_header_window_controls_linux(
    hdr: &adw::HeaderBar,
    baseline: &Rc<Cell<Option<(bool, bool)>>>,
    show_chrome: bool,
) {
    capture_linux_baseline(hdr, baseline, show_chrome);

    let (s_on, e_on) = baseline
        .get()
        .filter(|&(s, e)| s || e)
        .unwrap_or((true, true));
    apply_linux_title_buttons(hdr, s_on, e_on, show_chrome);
    hdr.queue_allocate();
}
