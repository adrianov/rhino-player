fn sync_volume(w: &TransportWidgets, vol: f64) {
    let muted = w.vol_mute.is_active();
    sync_volume_header_readout(w, muted, vol);
    if w.vol_menu.is_active() {
        return;
    }
    let clamped = vol.clamp(0.0, w.vol_adj.upper());
    if (w.vol_adj.value() - clamped).abs() < 0.01 {
        return;
    }
    w.vol_sync.set(true);
    w.vol_adj.set_value(clamped);
    w.vol_sync.set(false);
}

/// Header speaker icon + percent readout, derived from mute state and the linear slider value.
fn sync_volume_header_readout(w: &TransportWidgets, muted: bool, slider_lin: f64) {
    let v_icon = vol_icon(muted, slider_lin);
    if w.vol_header_img.icon_name().as_deref() != Some(v_icon) {
        w.vol_header_img.set_icon_name(Some(v_icon));
    }
    stamp_vol_percent_readout(&w.vol_readout, slider_lin, w.vol_adj.upper());
}

fn sync_mute(w: &TransportWidgets, muted: bool) {
    sync_mute_toggle(w, muted);
    set_tooltip_if_changed(
        w.vol_mute.upcast_ref::<gtk::Widget>(),
        if muted { "Unmute" } else { "Mute" },
    );
    let slider_lin = w.vol_adj.value();
    sync_volume_header_readout(w, muted, slider_lin);
}

fn sync_mute_toggle(w: &TransportWidgets, muted: bool) {
    let icon = vol_mute_pop_icon(muted);
    if w.vol_mute.icon_name().as_deref() != Some(icon) {
        w.vol_mute.set_icon_name(icon);
    }
    if w.vol_mute.is_active() != muted {
        w.vol_sync.set(true);
        w.vol_mute.set_active(muted);
        w.vol_sync.set(false);
    }
}

fn stamp_vol_percent_readout(l: &gtk::Label, linear: f64, vmax: f64) {
    let cap = if vmax.is_finite() && vmax > 0.0 {
        vmax
    } else {
        100.0
    };
    let v = linear.clamp(0.0, cap);
    let pct = ((v / cap) * 100.0).round().clamp(0.0, 100.0) as i32;
    let s = format!("{pct}%");
    if l.text().as_str() != s {
        l.set_text(&s);
    }
}

fn sync_volume_max(w: &TransportWidgets, vmax: f64) {
    if vmax.is_finite() && vmax > 0.0 && (w.vol_adj.upper() - vmax).abs() > f64::EPSILON {
        w.vol_adj.set_upper(vmax);
    }
    stamp_vol_percent_readout(&w.vol_readout, w.vol_adj.value(), w.vol_adj.upper());
}
