include!("chrome_macos_traffic_lights.rs");

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
    sync_header_window_controls_macos(hdr, baseline, show_chrome, root);
    #[cfg(not(target_os = "macos"))]
    sync_header_window_controls_linux(hdr, baseline, show_chrome);
    #[cfg(not(target_os = "macos"))]
    let _ = root;
}

/// macOS: native titlebar buttons — windowed hide/show timing in `chrome_macos_traffic_lights.rs`.
#[cfg(target_os = "macos")]
fn sync_header_window_controls_macos(
    hdr: &adw::HeaderBar,
    _baseline: &Rc<Cell<Option<(bool, bool)>>>,
    show_chrome: bool,
    root: &adw::ToolbarView,
) {
    use gtk::prelude::WidgetExt;

    let fullscreen = hdr
        .root()
        .and_then(|w| w.downcast::<adw::ApplicationWindow>().ok())
        .is_some_and(|win| win.is_fullscreen());

    macos_traffic_cancel_poll();

    if fullscreen {
        crate::macos_window::set_traffic_lights_visible(hdr, true);
        return;
    }

    if !show_chrome {
        crate::macos_window::set_traffic_lights_visible(hdr, false);
        return;
    }

    crate::macos_window::set_traffic_lights_visible(hdr, false);
    macos_traffic_poll_until_stable(hdr.clone(), root.clone());
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
