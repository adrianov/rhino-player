/// Builds the sibling-navigation context for the macOS Now Playing remote straight from
/// [WindowAfterPresentArgs] — field names carry meaning, no positional tuple hops.
#[cfg(target_os = "macos")]
fn wap_sibling_nav_ctx(args: &WindowAfterPresentArgs) -> SiblingNavCtx {
    SiblingNavCtx {
        btn_prev: args.w.sibling_nav.prev_btn.clone(),
        btn_next: args.w.sibling_nav.next_btn.clone(),
        player: args.player.clone(),
        win: args.w.win.clone(),
        gl: args.w.gl_area.clone(),
        recent: args.w.recent_scrl.clone(),
        last_path: args.last_path.clone(),
        video_pref: args.video_pref.clone(),
        on_video_chrome: args.on_video_chrome.clone(),
        win_aspect: args.win_aspect.clone(),
        sibling_seof: args.sibling_seof.clone(),
        on_file_loaded: args.on_file_loaded.clone(),
        hdr_title_mirror: args.hdr_title_mirror.clone(),
        playback_focus: args.playback_focus.clone(),
        on_open_fail: args.on_open_fail.clone(),
    }
}

/// macOS Now Playing / media-key remote wired to the sibling navigation.
#[cfg(target_os = "macos")]
fn wire_macos_now_playing_step(args: &WindowAfterPresentArgs) {
    wire_macos_now_playing_remote(args.play_ctx.clone(), wap_sibling_nav_ctx(args));
}
