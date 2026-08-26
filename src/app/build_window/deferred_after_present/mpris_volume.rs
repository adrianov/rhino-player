/// Volume slider / mute / readout wiring step of [wire_window_after_present].
fn wire_volume_controls_step(args: &WindowAfterPresentArgs) {
    wire_volume_controls(VolumeCtx {
        player: args.player.clone(),
        recent: args.w.recent_scrl.clone(),
        gl: args.w.gl_area.clone(),
        vol_header_img: args.w.vol_header_img.clone(),
        vol_readout: args.w.vol_readout.clone(),
        vol_adj: args.w.vol_adj.clone(),
        vol_mute_btn: args.w.vol_mute_btn.clone(),
        vol_sync: args.vol_sync.clone(),
    });
}

/// MPRIS D-Bus wiring after the seek control exists (Linux only).
#[cfg(target_os = "linux")]
fn wire_mpris_linux_step(args: &WindowAfterPresentArgs) {
    wire_mpris_linux_after_seek(MprisLinuxWireCtx {
        app: &args.app,
        win: args.w.win.clone(),
        gl_area: args.w.gl_area.clone(),
        recent_scrl: args.w.recent_scrl.clone(),
        player: &args.player,
        play_ctx: &args.play_ctx,
        last_path: &args.last_path,
        win_aspect: &args.win_aspect,
        sibling_seof: &args.sibling_seof,
        video_pref: Rc::clone(&args.play_ctx.video_pref),
        smooth_seek_debounce: args.smooth_seek_debounce.clone(),
        resume_after_seek_idle: args.resume_after_seek_idle.clone(),
        dvd_bar: Rc::clone(&args.dvd_bar),
        on_file_loaded: &args.on_file_loaded,
        on_video_chrome: &args.on_video_chrome,
        hdr_title_mirror: args.hdr_title_mirror.clone(),
        playback_focus: &args.playback_focus,
        on_open_fail: &args.on_open_fail,
    });
}
