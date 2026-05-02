{
    wire_seek_control(&w.seek, SeekControlDeps {
        player: player.clone(),
        gl: w.gl_area.clone(),
        seek_sync: seek_sync.clone(),
        seek_grabbed: seek_grabbed.clone(),
        time_left: w.time_left.clone(),
        preview_hover_t: seek_preview.hover_t.clone(),
        reapply_60: reapply_60.clone(),
    });

    #[cfg(target_os = "linux")]
    wire_mpris_linux_after_seek(
        app,
        w.win.clone(),
        w.gl_area.clone(),
        w.recent_scrl.clone(),
        player,
        &play_ctx,
        &last_path,
        &win_aspect,
        &sibling_seof,
        reapply_60.clone(),
        &on_file_loaded,
        &on_video_chrome,
    );
}
