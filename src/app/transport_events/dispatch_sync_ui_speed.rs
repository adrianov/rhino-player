fn sync_speed_header(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    w: &TransportWidgets,
    dur: f64,
) {
    let has_media = dur > 0.0;
    if w.speed_menu.is_sensitive() != has_media {
        w.speed_menu.set_sensitive(has_media);
    }
    #[cfg(target_os = "macos")]
    if crate::macos_header_menu::any_open() {
        return;
    }
    if !has_media {
        playback_speed::stamp_speed_readout(&w.speed_readout, 1.0);
        return;
    }
    with_bundle(player, |b| {
        let s = b.mpv.get_property::<f64>("speed").unwrap_or(1.0);
        let (_, canon) = playback_speed::nearest(s);
        playback_speed::stamp_speed_readout(&w.speed_readout, canon);
    });
}
