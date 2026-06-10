include!("dispatch_sync_ui_smooth_resync.rs");

fn with_bundle(player: &Rc<RefCell<Option<MpvBundle>>>, f: impl FnOnce(&MpvBundle)) {
    if let Ok(g) = player.try_borrow() {
        if let Some(b) = g.as_ref() {
            f(b);
        }
    }
}

fn has_open_path(mpv: &Mpv) -> bool {
    matches!(mpv.get_property::<String>("path"), Ok(s) if !s.trim().is_empty())
}

include!("dispatch_sync_ui_dispatch.rs");
include!("dispatch_sync_ui_file_loaded.rs");
include!("dispatch_sync_ui_dvd_bar.rs");
include!("dispatch_sync_ui_speed.rs");
include!("dispatch_sync_ui_volume.rs");

fn sync_play_button(w: &TransportWidgets, dur: f64, paused: bool) {
    let has_media = dur > 0.0;
    if w.play_pause.is_sensitive() != has_media {
        w.play_pause.set_sensitive(has_media);
    }
    let (icon, tip) = if has_media && !paused {
        ("media-playback-pause-symbolic", "Pause (Space)")
    } else if has_media {
        ("media-playback-start-symbolic", "Play (Space)")
    } else {
        ("media-playback-start-symbolic", "No media")
    };
    if w.play_pause.icon_name().as_deref() != Some(icon) {
        w.play_pause.set_icon_name(icon);
    }
    set_tooltip_if_changed(w.play_pause.upcast_ref::<gtk::Widget>(), tip);
}

fn sync_seek_range(w: &TransportWidgets, dur: f64) {
    let has_media = dur > 0.0;
    if w.seek.is_sensitive() != has_media {
        w.seek.set_sensitive(has_media);
    }
    if has_media && (w.seek_adj.upper() - dur).abs() > f64::EPSILON {
        w.seek_sync.set(true);
        w.seek_adj.set_lower(0.0);
        w.seek_adj.set_upper(dur);
        w.seek_sync.set(false);
    }
}

fn sync_seek_pos(w: &TransportWidgets, pos: f64, dur: f64) {
    if dur <= 0.0 || !pos.is_finite() || w.seek_grabbed.get() {
        return;
    }
    let v = pos.clamp(0.0, dur);
    if (w.seek_adj.value() - v).abs() < 0.01 {
        return;
    }
    w.seek_sync.set(true);
    w.seek_adj.set_value(v);
    w.seek_sync.set(false);
}

fn update_time_labels(w: &TransportWidgets, pos: f64, _dur: f64) {
    if w.seek_grabbed.get() {
        return;
    }
    let pos_s = format_time(pos);
    if w.time_left.text().as_str() != pos_s {
        w.time_left.set_text(&pos_s);
    }
}

fn sync_duration_label(w: &TransportWidgets, dur: f64) {
    let dur_s = format_time(dur);
    if w.time_right.text().as_str() != dur_s {
        w.time_right.set_text(&dur_s);
    }
}

fn set_tooltip_if_changed(w: &gtk::Widget, tip: &str) {
    if w.tooltip_text().as_deref() != Some(tip) {
        w.set_tooltip_text(Some(tip));
    }
}

fn sync_audio_tooltip(ctx: &Rc<TransportCtx>) {
    audio_tracks::refresh_audio_tooltip_for_player(&ctx.player, &ctx.widgets.vol_menu);
}
