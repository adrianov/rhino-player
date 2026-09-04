include!("wire_handlers_before_mpv_shell.rs");

// Phase orchestration for [wire_handlers_before_mpv]: bundles long-lived refs, runs each
// pre-MPV wiring phase in the original order, and collects what the caller must keep alive.

/// Long-lived window state referenced across every pre-MPV phase.
struct PreMpvPhaseRefs<'a> {
    app: &'a adw::Application,
    w: &'a WindowWidgets,
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    file_boot: &'a Rc<RefCell<Option<PathBuf>>>,
    on_open_slot: &'a Rc<RefCell<Option<RcPathFn>>>,
    sub_pref: &'a Rc<RefCell<db::SubPrefs>>,
    video_pref: &'a Rc<RefCell<db::VideoPrefs>>,
    bar_show: &'a Rc<Cell<bool>>,
    nav_t: &'a Rc<RefCell<Option<glib::SourceId>>>,
    motion_squelch: &'a Rc<Cell<Option<Instant>>>,
    playback_focus: &'a Rc<Cell<bool>>,
    win_aspect: &'a Rc<WinAspectCell>,
    last_path: &'a Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: &'a Rc<SiblingEofState>,
    dvd_bar: &'a Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    fs_restore: &'a Rc<RefCell<Option<(i32, i32)>>>,
    last_unmax: &'a Rc<RefCell<(i32, i32)>>,
    skip_max_to_fs: &'a Rc<Cell<bool>>,
    fs_transition_busy: &'a Rc<Cell<bool>>,
}

/// State produced by the pre-MPV phases that outlives this call.
struct BeforeMpvHandles {
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    vc: VideoChromeHandles,
    on_file_loaded: Rc<dyn Fn()>,
    close_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    trash_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    media_open: MediaOpenWire,
    browse: ContinueBrowse,
}

type SeekCells = (
    Rc<Cell<bool>>,
    Rc<Cell<bool>>,
    Rc<RefCell<Option<glib::SourceId>>>,
    Rc<Cell<bool>>,
);

type ActionCellPair = (
    Rc<RefCell<Option<gio::SimpleAction>>>,
    Rc<RefCell<Option<gio::SimpleAction>>>,
);

fn new_seek_cells() -> SeekCells {
    (
        Rc::new(Cell::new(false)),
        Rc::new(Cell::new(false)),
        Rc::new(RefCell::new(None::<glib::SourceId>)),
        Rc::new(Cell::new(false)),
    )
}

fn new_action_cell_pair() -> ActionCellPair {
    (Rc::new(RefCell::new(None)), Rc::new(RefCell::new(None)))
}

/// Notice toast + continue-grid browse preparation; also yields its open-failure reporter.
fn start_continue_browse(
    w: &WindowWidgets,
    playback_focus: &Rc<Cell<bool>>,
) -> (Rc<dyn Fn(String)>, ContinueBrowsePrep) {
    let prep = ContinueBrowsePrep::start(
        crate::recent_view::NoticeToastCtrl::new(w.notice_toast.clone()),
        w.recent_scrl.clone(),
        Rc::clone(playback_focus),
    );
    (Rc::clone(&prep.on_open_fail), prep)
}

fn attach_media_open(
    r: &PreMpvPhaseRefs<'_>,
    vc: &VideoChromeHandles,
    on_file_loaded: &Rc<dyn Fn()>,
    on_open_fail: &Rc<dyn Fn(String)>,
) -> MediaOpenWire {
    MediaOpenWire::attach(MediaOpenParts {
        app: r.app,
        w: r.w,
        player: r.player,
        video_pref: r.video_pref,
        last_path: r.last_path,
        on_video_chrome: Rc::clone(&vc.on_show),
        on_file_loaded: Rc::clone(on_file_loaded),
        win_aspect: r.win_aspect,
        playback_focus: r.playback_focus,
        sibling_seof: r.sibling_seof,
        on_open_fail: Rc::clone(on_open_fail),
        on_open_slot: r.on_open_slot,
    })
}

fn finish_continue_browse(
    prep: ContinueBrowsePrep,
    r: &PreMpvPhaseRefs<'_>,
    want_recent: bool,
    media_open: &MediaOpenWire,
    vc: &VideoChromeHandles,
    close_action_cell: &Rc<RefCell<Option<gio::SimpleAction>>>,
) -> ContinueBrowse {
    prep.finish(ContinueBrowseFinish {
        want_recent,
        player: r.player,
        video_pref: r.video_pref,
        w: r.w,
        last_path: r.last_path,
        on_open: media_open.on_open.clone(),
        sibling_seof: r.sibling_seof,
        win_aspect: r.win_aspect,
        playback_focus: r.playback_focus,
        close_action_cell: Rc::clone(close_action_cell),
        dvd_bar: r.dvd_bar,
        hdr_csd_baseline: Rc::clone(&vc.hdr_csd_baseline),
        nav_t: r.nav_t,
        bar_show: r.bar_show,
    })
}

/// Runs every pre-MPV phase in the established order and gathers the resulting handles.
fn wire_pre_mpv_phases(r: PreMpvPhaseRefs<'_>) -> BeforeMpvHandles {
    wire_header_cluster(r.w, r.player, r.sub_pref);
    let (seek_sync, seek_grabbed, smooth_seek_debounce, resume_after_seek_idle) = new_seek_cells();

    let (on_open_fail, browse_prep) = start_continue_browse(r.w, r.playback_focus);
    let (close_action_cell, trash_action_cell) = new_action_cell_pair();

    let on_file_loaded = wire_file_loaded_and_sub_style(&r, &close_action_cell, &trash_action_cell);

    wire_fullscreen_toggles(&r);

    let want_recent = set_recent_strip_visible(r.w, r.file_boot);
    let vc = attach_video_chrome_handles(
        r.w,
        r.player,
        r.bar_show,
        r.nav_t,
        r.motion_squelch,
        &seek_grabbed,
    );

    let media_open = attach_media_open(&r, &vc, &on_file_loaded, &on_open_fail);

    BeforeMpvHandles {
        seek_sync,
        seek_grabbed,
        smooth_seek_debounce,
        resume_after_seek_idle,
        on_file_loaded,
        trash_action_cell,
        browse: finish_continue_browse(
            browse_prep,
            &r,
            want_recent,
            &media_open,
            &vc,
            &close_action_cell,
        ),
        close_action_cell,
        vc,
        media_open,
    }
}
