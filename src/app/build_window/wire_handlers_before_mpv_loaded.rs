// File-loaded handler + subtitle-style controls, and the video-chrome handle extraction
// consumed by [wire_pre_mpv_phases].

type LoadedCore = (
    Rc<RefCell<Option<MpvBundle>>>,
    Rc<RefCell<Option<PathBuf>>>,
    Rc<SiblingEofState>,
    Rc<RefCell<db::SubPrefs>>,
);

impl PreMpvPhaseRefs<'_> {
    /// Cloned (gl area, recent strip, bottom bar) shared by loaded/sub-style wiring.
    fn shared_panels(&self) -> (gtk::GLArea, gtk::Box, gtk::Box) {
        (
            self.w.gl_area.clone(),
            self.w.recent_scrl.clone(),
            self.w.bottom.clone(),
        )
    }

    /// Cloned (player, last path, sibling EOF state, subtitle prefs).
    fn loaded_core(&self) -> LoadedCore {
        (
            self.player.clone(),
            self.last_path.clone(),
            self.sibling_seof.clone(),
            self.sub_pref.clone(),
        )
    }

    /// Cloned speed-menu widgets: sync flag, menu button, list box, readout label.
    fn speed_cluster(&self) -> (Rc<Cell<bool>>, gtk::MenuButton, gtk::ListBox, gtk::Label) {
        (
            self.w.speed_sync.clone(),
            self.w.speed_mbtn.clone(),
            self.w.speed_list.clone(),
            self.w.speed_readout.clone(),
        )
    }

    /// Cloned subtitle-style controls: scale adjustment and color button.
    fn sub_style_extras(&self) -> (gtk::Adjustment, gtk::ColorDialogButton) {
        (self.w.sub_scale_adj.clone(), self.w.sub_color_btn.clone())
    }
}

fn make_file_loaded_ctx(
    r: &PreMpvPhaseRefs<'_>,
    close_action_cell: &Rc<RefCell<Option<gio::SimpleAction>>>,
    trash_action_cell: &Rc<RefCell<Option<gio::SimpleAction>>>,
) -> FileLoadedCtx {
    let (gl, recent, bottom) = r.shared_panels();
    let (player, last_path, sibling_seof, sub_pref) = r.loaded_core();
    let (speed_sync, speed_menu, speed_list, speed_readout) = r.speed_cluster();
    FileLoadedCtx {
        player,
        last_path,
        sibling_seof,
        sibling_nav: r.w.sibling_nav.clone(),
        sub_pref,
        gl,
        bar_show: r.bar_show.clone(),
        recent,
        bottom,
        sub_menu: r.w.sub_menu.clone(),
        close_action_cell: Rc::clone(close_action_cell),
        trash_action_cell: Rc::clone(trash_action_cell),
        speed_sync,
        speed_menu,
        speed_list,
        speed_readout,
        video_pref: Rc::clone(r.video_pref),
        app: r.app.clone(),
        close_video_btn: r.w.close_video_btn.clone(),
    }
}

fn make_sub_style_ctx(r: &PreMpvPhaseRefs<'_>) -> SubStyleCtx {
    let (gl, recent, bottom) = r.shared_panels();
    let (sub_scale_adj, sub_color_btn) = r.sub_style_extras();
    SubStyleCtx {
        player: r.player.clone(),
        sub_pref: r.sub_pref.clone(),
        gl,
        bar_show: r.bar_show.clone(),
        recent,
        bottom,
        sub_scale_adj,
        sub_color_btn,
    }
}

fn wire_file_loaded_and_sub_style(
    r: &PreMpvPhaseRefs<'_>,
    close_action_cell: &Rc<RefCell<Option<gio::SimpleAction>>>,
    trash_action_cell: &Rc<RefCell<Option<gio::SimpleAction>>>,
) -> Rc<dyn Fn()> {
    let on_file_loaded = make_file_loaded_handler(make_file_loaded_ctx(
        r,
        close_action_cell,
        trash_action_cell,
    ));
    wire_sub_style_controls(make_sub_style_ctx(r));
    on_file_loaded
}

/// Fields of [VideoChrome] the rest of window wiring needs.
struct VideoChromeHandles {
    hdr_csd_baseline: Rc<Cell<Option<(bool, bool)>>>,
    ch_hide: Rc<ChromeBarHide>,
    on_show: Rc<dyn Fn()>,
}

fn attach_video_chrome_handles(
    w: &WindowWidgets,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    bar_show: &Rc<Cell<bool>>,
    nav_t: &Rc<RefCell<Option<glib::SourceId>>>,
    motion_squelch: &Rc<Cell<Option<Instant>>>,
    seek_grabbed: &Rc<Cell<bool>>,
) -> VideoChromeHandles {
    let vc = VideoChrome::attach(VideoChromeParts {
        win: &w.win,
        root: &w.root,
        header: &w.header,
        outer_ovl: &w.outer_ovl,
        video_handle: &w.video_handle,
        gl: &w.gl_area,
        recent: &w.recent_scrl,
        bottom: &w.bottom,
        #[cfg(target_os = "macos")]
        bottom_shell: &w.bottom_shell,
        player,
        bar_show,
        nav_t,
        motion_squelch,
        seek_grabbed,
        vol_menu: &w.vol_menu,
        sub_menu: &w.sub_menu,
        speed_mbtn: &w.speed_mbtn,
        menu_btn: &w.menu_btn,
    });
    VideoChromeHandles {
        hdr_csd_baseline: vc.hdr_csd_baseline,
        ch_hide: vc.ch_hide,
        on_show: vc.on_show,
    }
}
