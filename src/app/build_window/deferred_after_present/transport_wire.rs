/// Transport core refs, group A (indexed tuples keep the assembly under AbcSize budgets).
type WapTransportCoreA = (
    Rc<RefCell<Option<MpvBundle>>>,
    Rc<RefCell<db::VideoPrefs>>,
    Rc<RefCell<db::SubPrefs>>,
    adw::ApplicationWindow,
    gtk::GLArea,
    gtk::Box,
    Rc<Cell<bool>>,
    Rc<RefCell<Option<PathBuf>>>,
    Rc<SiblingEofState>,
    SiblingNavUi,
);

fn wap_transport_core_a(args: &WindowAfterPresentArgs) -> WapTransportCoreA {
    (
        args.player.clone(),
        args.video_pref.clone(),
        args.sub_pref.clone(),
        args.w.win.clone(),
        args.w.gl_area.clone(),
        args.w.recent_scrl.clone(),
        args.recent_visible.clone(),
        args.last_path.clone(),
        args.sibling_seof.clone(),
        args.w.sibling_nav.clone(),
    )
}

/// Transport core refs, group B.
type WapTransportCoreB = (
    Rc<Cell<bool>>,
    Rc<WinAspectCell>,
    Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    Rc<Cell<bool>>,
    Rc<dyn Fn()>,
    Rc<dyn Fn()>,
    VideoReapply60,
    Option<Rc<gtk::Label>>,
);

fn wap_transport_core_b(args: &WindowAfterPresentArgs) -> WapTransportCoreB {
    (
        args.exit_after_current.clone(),
        args.win_aspect.clone(),
        args.idle_inhib.clone(),
        args.mpv_teardown_after_draw.clone(),
        args.on_video_chrome.clone(),
        args.on_file_loaded.clone(),
        args.reapply_60.clone(),
        args.hdr_title_mirror.clone(),
    )
}

/// Transport core refs, group C.
type WapTransportCoreC = (
    Rc<Cell<bool>>,
    Rc<Cell<bool>>,
    Rc<RefCell<Vec<(f64, String)>>>,
    Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    Rc<crate::screen_blackout::BlackoutSync>,
    crate::media_probe::ContinueGridCache,
    Rc<dyn Fn(String)>,
    adw::Application,
);

fn wap_transport_core_c(args: &WindowAfterPresentArgs) -> WapTransportCoreC {
    (
        args.bar_show.clone(),
        args.playback_focus.clone(),
        args.seek_chapters.clone(),
        args.dvd_bar.clone(),
        Rc::clone(&args.w.blackout_sync),
        args.continue_grid_cache.clone(),
        args.on_open_fail.clone(),
        args.app.clone(),
    )
}

/// Toolbar transport widget refs, group A (seek / speed cluster).
type WapTransportWidgetsA = (
    gtk::Button,
    gtk::Scale,
    gtk::Adjustment,
    Rc<Cell<bool>>,
    Rc<Cell<bool>>,
    gtk::Label,
    gtk::Label,
    gtk::MenuButton,
    gtk::Label,
);

fn wap_transport_widgets_a(args: &WindowAfterPresentArgs) -> WapTransportWidgetsA {
    (
        args.w.play_pause.clone(),
        args.w.seek.clone(),
        args.w.seek_adj.clone(),
        args.seek_sync.clone(),
        args.seek_grabbed.clone(),
        args.w.time_left.clone(),
        args.w.time_right.clone(),
        args.w.speed_mbtn.clone(),
        args.w.speed_readout.clone(),
    )
}

/// Toolbar transport widget refs, group B (volume / subtitles / smooth cluster).
type WapTransportWidgetsB = (
    gtk::MenuButton,
    gtk::Image,
    gtk::Label,
    gtk::Adjustment,
    gtk::ToggleButton,
    Rc<Cell<bool>>,
    gtk::Label,
    gtk::Button,
    gtk::Label,
);

fn wap_transport_widgets_b(args: &WindowAfterPresentArgs) -> WapTransportWidgetsB {
    (
        args.w.vol_menu.clone(),
        args.w.vol_header_img.clone(),
        args.w.vol_readout.clone(),
        args.w.vol_adj.clone(),
        args.w.vol_mute_btn.clone(),
        args.vol_sync.clone(),
        args.w.sub_readout.clone(),
        args.w.smooth_btn.clone(),
        args.w.smooth_status.clone(),
    )
}

fn transport_widgets_step(args: &WindowAfterPresentArgs) -> TransportWidgets {
    let wa = wap_transport_widgets_a(args);
    let wb = wap_transport_widgets_b(args);
    TransportWidgets {
        play_pause: wa.0,
        seek: wa.1,
        seek_adj: wa.2,
        seek_sync: wa.3,
        seek_grabbed: wa.4,
        time_left: wa.5,
        time_right: wa.6,
        speed_menu: wa.7,
        speed_readout: wa.8,
        vol_menu: wb.0,
        vol_header_img: wb.1,
        vol_readout: wb.2,
        vol_adj: wb.3,
        vol_mute: wb.4,
        vol_sync: wb.5,
        sub_readout: wb.6,
        smooth_toolbar_btn: wb.7,
        smooth_toolbar_status: wb.8,
    }
}

/// Transport events / tick wiring step of [wire_window_after_present].
fn wire_transport_events_step(args: &WindowAfterPresentArgs) {
    let ca = wap_transport_core_a(args);
    let cb = wap_transport_core_b(args);
    let cc = wap_transport_core_c(args);
    wire_transport_events(TransportSetup {
        app: cc.7,
        player: ca.0,
        video_pref: ca.1,
        sub_pref: ca.2,
        win: ca.3,
        gl: ca.4,
        recent: ca.5,
        recent_visible: ca.6,
        last_path: ca.7,
        sibling_seof: ca.8,
        sibling_nav: ca.9,
        exit_after_current: cb.0,
        win_aspect: cb.1,
        idle_inhib: cb.2,
        mpv_teardown_after_draw: cb.3,
        on_video_chrome: cb.4,
        on_file_loaded: cb.5,
        reapply_60: cb.6,
        hdr_title_mirror: cb.7,
        bar_show: cc.0,
        playback_focus: cc.1,
        widgets: transport_widgets_step(args),
        seek_chapters: cc.2,
        dvd_bar: cc.3,
        blackout: cc.4,
        continue_grid_cache: cc.5,
        on_open_fail: cc.6,
    });
}
