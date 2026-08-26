// Grouped handle snapshots consumed by `keys_dispatch.rs`: each key family gets the refs it
// needs, cloned once at controller-wiring time.

/// Fullscreen-toggle family (Enter / KP_Enter / `F`): geometry + transition latches.
struct FullscreenKeyRefs {
    fr: Rc<RefCell<Option<(i32, i32)>>>,
    lu: Rc<RefCell<(i32, i32)>>,
    skip: Rc<Cell<bool>>,
    fs_busy: Rc<Cell<bool>>,
}

impl FullscreenKeyRefs {
    fn new(ctx: &WindowInputCtx) -> Self {
        Self {
            fr: ctx.fs_restore.clone(),
            lu: ctx.last_unmax.clone(),
            skip: ctx.skip_max_to_fs.clone(),
            fs_busy: Rc::clone(&ctx.fs_transition_busy),
        }
    }
}

/// Horizontal-seek arrow family: seek bar + debounce/timer slots.
struct SeekArrowKeys {
    seek: gtk::Scale,
    seek_sync: Rc<Cell<bool>>,
    time_left: gtk::Label,
    gl: gtk::GLArea,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    play_toggle: PlayToggleCtx,
}

impl SeekArrowKeys {
    fn new(ctx: &WindowInputCtx) -> Self {
        Self {
            seek: ctx.seek.clone(),
            seek_sync: ctx.seek_sync.clone(),
            time_left: ctx.time_left.clone(),
            gl: ctx.shell.gl.clone(),
            smooth_seek_debounce: ctx.smooth_seek_debounce.clone(),
            dvd_bar: Rc::clone(&ctx.dvd_bar),
            resume_after_seek_idle: ctx.resume_after_seek_idle.clone(),
            play_toggle: ctx.play_toggle.clone(),
        }
    }
}

/// Sibling-navigation snapshot (media keys + Ctrl+arrows): rebuilt per press as
/// [`SiblingNavTryRefs`] via [`KeyDispatch::nav_refs`].
struct NavHandleSnapshot {
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    win_aspect: Rc<WinAspectCell>,
    sibling_seof: Rc<SiblingEofState>,
    on_file_loaded: Rc<dyn Fn()>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
    on_open_fail: Rc<dyn Fn(String)>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
}

impl NavHandleSnapshot {
    fn new(ctx: &WindowInputCtx) -> Self {
        Self {
            last_path: ctx.last_path.clone(),
            on_video_chrome: ctx.on_video_chrome.clone(),
            win_aspect: ctx.win_aspect.clone(),
            sibling_seof: ctx.sibling_seof.clone(),
            on_file_loaded: ctx.on_file_loaded.clone(),
            hdr_title_mirror: ctx.hdr_title_mirror.clone(),
            playback_focus: Rc::clone(&ctx.playback_focus),
            on_open_fail: Rc::clone(&ctx.on_open_fail),
            video_pref: ctx.video_pref.clone(),
        }
    }
}

/// [`PlayToggleCtx`] wired for keyboard play/pause shortcuts.
fn play_toggle_ctx_for_keys(
    ctx: &WindowInputCtx,
    p: &Rc<RefCell<Option<MpvBundle>>>,
    win_key: &adw::ApplicationWindow,
    recent_esc: &gtk::Box,
) -> PlayToggleCtx {
    PlayToggleCtx {
        app: ctx.app.clone(),
        player: p.clone(),
        video_pref: Rc::clone(&ctx.video_pref),
        win: win_key.clone(),
        video_handle: ctx.shell.video_handle.clone(),
        gl: ctx.shell.gl.clone(),
        recent: recent_esc.clone(),
        last_path: ctx.last_path.clone(),
        on_video_chrome: ctx.on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&ctx.on_file_loaded),
        win_aspect: ctx.win_aspect.clone(),
        sub_menu: None,
        play_pause: ctx.play_pause.clone(),
        hdr_title_mirror: ctx.hdr_title_mirror.clone(),
        playback_focus: Rc::clone(&ctx.playback_focus),
        incomplete_hold: Rc::clone(&ctx.sibling_seof.incomplete_hold),
    }
}

/// [`DigitSpeedShortcutCtx`] wired for digit speed shortcuts.
fn digit_speed_ctx_for_keys(
    ctx: &WindowInputCtx,
    p: &Rc<RefCell<Option<MpvBundle>>>,
) -> DigitSpeedShortcutCtx {
    DigitSpeedShortcutCtx {
        player: p.clone(),
        play_toggle: ctx.play_toggle.clone(),
        gl: ctx.shell.gl.clone(),
        video_pref: Rc::clone(&ctx.video_pref),
        app: ctx.app.clone(),
        speed_sync: ctx.speed_sync.clone(),
        speed_menu: ctx.speed_menu.clone(),
        speed_list: ctx.speed_list.clone(),
        speed_readout: ctx.speed_readout.clone(),
    }
}
