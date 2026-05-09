/// [`gio::Menu`] refs and header widgets threaded into [`register_video_app_actions`].
struct VideoAppMenuWire {
    pref_menu: gio::Menu,
    seek_bar_on: Rc<Cell<bool>>,
    smooth_toolbar_status: Option<gtk::Label>,
}

/// Header readout: rounded **output** frame rate (`estimated-vf-fps` when known, else
/// `container-fps`×`speed`). **—** when no file is open.
fn smooth_toolbar_readout_from_mpv(mpv: &Mpv) -> String {
    const LO: f64 = 0.05;
    const HI: f64 = 960.0;
    if !matches!(mpv.get_property::<String>("path"), Ok(s) if !s.trim().is_empty()) {
        return "—".to_string();
    }
    let spd_raw = mpv.get_property::<f64>("speed").unwrap_or(1.0);
    let spd = if spd_raw.is_finite() && (0.01..=8.0).contains(&spd_raw) {
        spd_raw.max(LO)
    } else {
        1.0
    };
    if let Ok(est) = mpv.get_property::<f64>("estimated-vf-fps") {
        if est.is_finite() && est > LO && est < HI {
            return format!("{}", est.round() as i64);
        }
    }
    let nominal = mpv.get_property::<f64>("container-fps").unwrap_or(0.0);
    if nominal.is_finite() && nominal > LO && nominal < HI {
        let v = (nominal * spd).round() as i64;
        return format!("{v}");
    }
    "—".to_string()
}

fn stamp_smooth_toolbar_readout(lab: Option<&gtk::Label>, player: &Rc<RefCell<Option<MpvBundle>>>) {
    let Some(l) = lab else {
        return;
    };
    let text = if let Ok(g) = player.try_borrow() {
        g.as_ref()
            .map(|b| smooth_toolbar_readout_from_mpv(&b.mpv))
            .unwrap_or_else(|| "—".to_string())
    } else {
        return;
    };
    l.set_label(&text);
}
