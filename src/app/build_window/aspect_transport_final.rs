{
    wire_aspect_resize_on_map(
        &w.win, &w.recent_scrl, &win_aspect, &aspect_resize_end_deb, &aspect_resize_wired,
    );

    wire_transport_events(TransportSetup {
        app: app.clone(), player: player.clone(),
        sub_pref: sub_pref.clone(),
        win: w.win.clone(), gl: w.gl_area.clone(), recent: w.recent_scrl.clone(),
        recent_visible,
        last_path: last_path.clone(), sibling_seof: sibling_seof.clone(),
        sibling_nav: w.sibling_nav.clone(), exit_after_current: exit_after_current.clone(),
        win_aspect: win_aspect.clone(), idle_inhib: Rc::clone(&idle_inhib),
        mpv_teardown_after_draw: Rc::clone(&mpv_teardown_after_draw),
        on_video_chrome: on_video_chrome.clone(), on_file_loaded: Rc::clone(&on_file_loaded),
        reapply_60: reapply_60.clone(), bar_show: bar_show.clone(),
        seek_chapters: Rc::clone(&seek_chapters),
        widgets: TransportWidgets {
            play_pause: w.play_pause.clone(), seek: w.seek.clone(), seek_adj: w.seek_adj.clone(),
            seek_sync: seek_sync.clone(), seek_grabbed: seek_grabbed.clone(),
            time_left: w.time_left.clone(), time_right: w.time_right.clone(),
            speed_menu: w.speed_mbtn.clone(), vol_menu: w.vol_menu.clone(),
            vol_adj: w.vol_adj.clone(), vol_mute: w.vol_mute_btn.clone(),
            vol_sync: vol_sync.clone(),
        },
    });

    wire_final_actions(FinalActionCtx {
        app: app.clone(),
        win: w.win.clone(),
        fs_restore: Rc::clone(&fs_restore),
        last_unmax: Rc::clone(&last_unmax),
        skip_max_to_fs: Rc::clone(&skip_max_to_fs),
        root: w.root.clone(),
        header: w.header.clone(),
        gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(), bottom: w.bottom.clone(), player: player.clone(),
        sub_pref: sub_pref.clone(), video_pref: Rc::clone(&video_pref),
        main_menu: w.main_menu.clone(), pref_menu: w.pref_menu.clone(),
        seek_bar_on: Rc::clone(&seek_bar_on),
        last_path: last_path.clone(), on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        win_aspect: Rc::clone(&win_aspect), bar_show: bar_show.clone(),
        idle_inhib: Rc::clone(&idle_inhib), exit_after_current: exit_after_current.clone(),
        mpv_teardown_after_draw: Rc::clone(&mpv_teardown_after_draw),
        hdr_csd_baseline: Rc::clone(&hdr_csd_baseline),
    });
}