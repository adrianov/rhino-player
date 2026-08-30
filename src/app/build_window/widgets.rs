include!("widgets_core.rs");
include!("window_widgets.rs");
include!("widgets_bars.rs");

fn build_widgets(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
    exit_after_current: Rc<Cell<bool>>,
) -> WindowWidgets {
    #[cfg(target_os = "macos")]
    std::hint::black_box(exit_after_current.clone());
    #[cfg(not(target_os = "macos"))]
    let _ = &exit_after_current;

    let menus = AppMenus::build();
    let groups = WidgetGroups::build(app, player, video_pref, sub_pref);
    let buttons = HeaderButtons::build(&groups.win, player, &menus.pref_menu, &groups.recent_scrl);

    let shell = build_toolbar_header_shell(
        &buttons.menu_btn,
        &groups.pops.vol_menu,
        &groups.pops.sub_menu,
        &groups.smooth.smooth_btn,
        &groups.speed.speed_mbtn,
        &buttons.fill_btn,
        &buttons.blackout_menu,
    );
    let times = build_seek_and_time_row();
    let (bottom, close_video_btn) = build_bottom_bar(
        &groups.chrome.sibling_nav.prev_wrap,
        &groups.chrome.play_pause,
        &groups.chrome.sibling_nav.next_wrap,
        &times.time_left,
        &times.seek,
        &times.time_right,
    );
    #[cfg(target_os = "macos")]
    let bottom_shell = crate::macos_bottom_bar::wrap_row(&bottom);
    let video_handle = mount_video_overlay(&groups.gl_area, &groups.recent_scrl);
    crate::video_fill::bind_fill_viewport(&groups.gl_area);

    WindowWidgets {
        win: groups.win,
        root: shell.root,
        header: shell.header,
        outer_ovl: groups.outer_ovl,
        video_handle,
        gl_area: groups.gl_area,
        bottom,
        #[cfg(target_os = "macos")]
        bottom_shell,
        play_pause: groups.chrome.play_pause,
        sibling_nav: groups.chrome.sibling_nav,
        menu_btn: buttons.menu_btn,
        vol_menu: groups.pops.vol_menu,
        vol_header_img: groups.pops.vol_header_img,
        vol_readout: groups.pops.vol_readout,
        sub_menu: groups.pops.sub_menu,
        sub_readout: groups.pops.sub_readout,
        smooth_btn: groups.smooth.smooth_btn,
        smooth_status: groups.smooth.smooth_status,
        speed_mbtn: groups.speed.speed_mbtn,
        speed_readout: groups.speed.speed_readout,
        speed_list: groups.speed.speed_list,
        speed_sync: groups.speed.speed_sync,
        seek: times.seek,
        seek_adj: times.seek_adj,
        time_left: times.time_left,
        time_right: times.time_right,
        vol_adj: groups.pops.vol_adj,
        vol_mute_btn: groups.pops.vol_mute_btn,
        audio_tracks_box: groups.pops.audio_tracks_box,
        audio_tracks_block: groups.pops.audio_tracks_block,
        audio_tracks_section: groups.pops.audio_tracks_section,
        sub_tracks_box: groups.pops.sub_tracks_box,
        sub_tracks_block: groups.pops.sub_tracks_block,
        sub_tracks_section: groups.pops.sub_tracks_section,
        sub_scale_adj: groups.pops.sub_scale_adj,
        sub_color_btn: groups.pops.sub_color_btn,
        sub_color_row: groups.pops.sub_color_row,
        vol_pop: groups.pops.vol_pop,
        sub_pop: groups.pops.sub_pop,
        #[cfg(target_os = "macos")]
        main_menu: menus.menubar_model,
        pref_menu: menus.pref_menu,
        recent_scrl: groups.recent_scrl,
        flow_recent: groups.flow_recent,
        recent_spacers: groups.recent_spacers,
        undo_bar: groups.undo_bar,
        notice_toast: groups.notice_toast,
        sibling_search: groups.sibling_search,
        fs_clock: shell.fs_clock,
        hdr_title_mirror: shell.hdr_title_mirror,
        close_video_btn,
        blackout_sync: buttons.blackout_sync,
        _header_btn_heights: shell._header_btn_heights,
    }
}
