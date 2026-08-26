struct BwPrefsState {
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    reapply_60: VideoReapply60,
    exit_after_current: Rc<Cell<bool>>,
}

struct BwTimelineState {
    seek_chapters: Rc<RefCell<Vec<(f64, String)>>>,
    dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    bar_show: Rc<Cell<bool>>,
    nav_t: Rc<RefCell<Option<glib::SourceId>>>,
    cur_t: Rc<RefCell<Option<glib::SourceId>>>,
    ptr_in_gl: Rc<Cell<bool>>,
    motion_squelch: Rc<Cell<Option<Instant>>>,
    last_cap_xy: Rc<Cell<Option<(f64, f64)>>>,
    last_gl_xy: Rc<Cell<Option<(f64, f64)>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    playback_focus: Rc<Cell<bool>>,
    seek_bar_on: Rc<Cell<bool>>,
    sibling_seof: Rc<SiblingEofState>,
}

#[derive(Default)]
struct BwChromeState {
    fs_restore: Rc<RefCell<Option<(i32, i32)>>>,
    fs_pause_stash: Rc<RefCell<Option<bool>>>,
    fs_transition_busy: Rc<Cell<bool>>,
    fs_transition_settle: Rc<RefCell<Option<glib::SourceId>>>,
    skip_max_to_fs: Rc<Cell<bool>>,
    last_unmax: Rc<RefCell<(i32, i32)>>,
    win_aspect: Rc<Cell<Option<(i64, i64)>>>,
    aspect_resize_end_deb: Rc<RefCell<Option<glib::SourceId>>>,
    aspect_resize_wired: Rc<Cell<bool>>,
    idle_inhib: Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    mpv_teardown_after_draw: Rc<Cell<bool>>,
}

struct BwState {
    prefs: BwPrefsState,
    tl: BwTimelineState,
    chrome: BwChromeState,
}

fn make_bw_prefs(app: &adw::Application) -> BwPrefsState {
    let video_pref = Rc::new(RefCell::new(db::load_video()));
    let reapply_60 = VideoReapply60 {
        vp: Rc::clone(&video_pref),
        app: app.clone(),
    };
    BwPrefsState {
        sub_pref: Rc::new(RefCell::new(db::load_sub())),
        video_pref,
        reapply_60,
        exit_after_current: Rc::new(Cell::new(false)),
    }
}

fn make_bw_timeline() -> BwTimelineState {
    let (seek_chapters, dvd_bar, nav_t, cur_t) = make_timeline_slots_a();
    let (ptr_in_gl, motion_squelch, last_cap_xy, last_gl_xy) = make_timeline_slots_b();
    let (last_path, playback_focus) = make_timeline_slots_c();
    BwTimelineState {
        seek_chapters,
        dvd_bar,
        nav_t,
        cur_t,
        ptr_in_gl,
        motion_squelch,
        last_cap_xy,
        last_gl_xy,
        last_path,
        playback_focus,
        bar_show: Rc::new(Cell::new(true)),
        seek_bar_on: Rc::new(Cell::new(db::load_seek_bar_preview())),
        sibling_seof: make_sibling_seof(),
    }
}

fn make_sibling_seof() -> Rc<SiblingEofState> {
    Rc::new(SiblingEofState {
        done: Cell::new(false),
        nav_key: RefCell::new(None),
        nav_can_prev: Cell::new(false),
        nav_can_next: Cell::new(false),
        pos_min: Cell::new(0.0),
        pos_max: Cell::new(0.0),
        pos_tracked: Cell::new(false),
        incomplete_hold: crate::incomplete_download_eof::IncompleteEofHold::new(),
    })
}

type TimelineSlotsA = (
    Rc<RefCell<Vec<(f64, String)>>>,
    Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    Rc<RefCell<Option<glib::SourceId>>>,
    Rc<RefCell<Option<glib::SourceId>>>,
);

fn make_timeline_slots_a() -> TimelineSlotsA {
    (
        Rc::new(RefCell::new(Vec::<(f64, String)>::new())),
        Rc::new(RefCell::new(None::<crate::dvd_vob_timeline::DvdBarState>)),
        Rc::new(RefCell::new(None::<glib::SourceId>)),
        Rc::new(RefCell::new(None::<glib::SourceId>)),
    )
}

type TimelineSlotsB = (
    Rc<Cell<bool>>,
    Rc<Cell<Option<Instant>>>,
    Rc<Cell<Option<(f64, f64)>>>,
    Rc<Cell<Option<(f64, f64)>>>,
);

fn make_timeline_slots_b() -> TimelineSlotsB {
    (
        Rc::new(Cell::new(false)),
        Rc::new(Cell::new(None::<Instant>)),
        Rc::new(Cell::new(None::<(f64, f64)>)),
        Rc::new(Cell::new(None::<(f64, f64)>)),
    )
}

fn make_timeline_slots_c() -> (Rc<RefCell<Option<PathBuf>>>, Rc<Cell<bool>>) {
    (
        Rc::new(RefCell::new(None::<PathBuf>)),
        Rc::new(Cell::new(false)),
    )
}

fn make_bw_chrome() -> BwChromeState {
    BwChromeState {
        last_unmax: Rc::new(RefCell::new((WIN_INIT_W, WIN_INIT_H))),
        ..Default::default()
    }
}

fn make_bw_state(app: &adw::Application) -> BwState {
    BwState {
        prefs: make_bw_prefs(app),
        tl: make_bw_timeline(),
        chrome: make_bw_chrome(),
    }
}
