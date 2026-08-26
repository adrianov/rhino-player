// Linux MPRIS wiring: seeks, sibling prev/next, and transport controls handed to mprisd.

#[cfg(target_os = "linux")]
struct MprisLinuxWireCtx<'a> {
    app: &'a adw::Application,
    win: adw::ApplicationWindow,
    gl_area: gtk::GLArea,
    recent_scrl: gtk::Box,
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    play_ctx: &'a PlayToggleCtx,
    last_path: &'a Rc<RefCell<Option<PathBuf>>>,
    win_aspect: &'a Rc<WinAspectCell>,
    sibling_seof: &'a Rc<SiblingEofState>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    on_file_loaded: &'a Rc<dyn Fn()>,
    on_video_chrome: &'a Rc<dyn Fn()>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: &'a Rc<Cell<bool>>,
    on_open_fail: &'a Rc<dyn Fn(String)>,
}

/// Sibling-navigation state shared by the MPRIS Previous/Next handlers.
#[cfg(target_os = "linux")]
#[derive(Clone)]
struct MprisSiblingNav {
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
    gl_area: gtk::GLArea,
    recent_scrl: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    on_video_chrome: Rc<dyn Fn()>,
    win_aspect: Rc<WinAspectCell>,
    sibling_seof: Rc<SiblingEofState>,
    on_file_loaded: Rc<dyn Fn()>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
    on_open_fail: Rc<dyn Fn(String)>,
}

#[cfg(target_os = "linux")]
impl MprisSiblingNav {
    fn from_wire_ctx(ctx: &MprisLinuxWireCtx<'_>) -> Self {
        Self {
            player: ctx.player.clone(),
            win: ctx.win.clone(),
            gl_area: ctx.gl_area.clone(),
            recent_scrl: ctx.recent_scrl.clone(),
            last_path: ctx.last_path.clone(),
            video_pref: ctx.video_pref.clone(),
            on_video_chrome: ctx.on_video_chrome.clone(),
            win_aspect: ctx.win_aspect.clone(),
            sibling_seof: ctx.sibling_seof.clone(),
            on_file_loaded: ctx.on_file_loaded.clone(),
            hdr_title_mirror: ctx.hdr_title_mirror.clone(),
            playback_focus: ctx.playback_focus.clone(),
            on_open_fail: ctx.on_open_fail.clone(),
        }
    }

    /// Loads the previous/next sibling through the same path as the bottom-bar buttons.
    fn step(&self, pick: fn(&Path) -> Option<PathBuf>, log_tag: &'static str) {
        try_load_sibling_pick(
            pick,
            log_tag,
            &SiblingNavTryRefs {
                player: self.player.clone(),
                win: self.win.clone(),
                gl: self.gl_area.clone(),
                recent: self.recent_scrl.clone(),
                last_path: self.last_path.clone(),
                video_pref: self.video_pref.clone(),
                on_video_chrome: self.on_video_chrome.clone(),
                win_aspect: self.win_aspect.clone(),
                sibling_seof: self.sibling_seof.clone(),
                on_file_loaded: self.on_file_loaded.clone(),
                hdr_title_mirror: self.hdr_title_mirror.clone(),
                playback_focus: self.playback_focus.clone(),
                on_open_fail: self.on_open_fail.clone(),
            },
        );
    }
}

/// External/scale-style absolute seek with the shared keyframe-seek pipeline.
#[cfg(target_os = "linux")]
fn make_mpris_seek_abs(ctx: &MprisLinuxWireCtx<'_>) -> crate::mpris::MpvSeekAbs {
    let p_seek = ctx.player.clone();
    let gl_seek = ctx.gl_area.clone();
    let deb_seek = ctx.smooth_seek_debounce.clone();
    let resume_seek = ctx.resume_after_seek_idle.clone();
    let toggle_seek = ctx.play_ctx.clone();
    let bar_seek = ctx.dvd_bar.clone();
    crate::mpris::MpvSeekAbs(Rc::new(move |secs: &str| {
        main_player_seek_keyframes(
            &SeekKeyframeParams {
                player: &p_seek,
                gl: &gl_seek,
                smooth_seek_debounce: &deb_seek,
                resume_after_seek_idle: &resume_seek,
                play_toggle: &toggle_seek,
                dvd_bar: Some(&bar_seek),
            },
            SeekKeyframeKind::ScaleOrExternal,
            secs,
        );
    }))
}

#[cfg(target_os = "linux")]
fn mpris_toggle_play_pause(play_ctx: &PlayToggleCtx) -> Rc<dyn Fn()> {
    let ctx = play_ctx.clone();
    Rc::new(move || {
        let _ = toggle_play_pause(&ctx);
    })
}

#[cfg(target_os = "linux")]
fn mpris_pause_only(play_ctx: &PlayToggleCtx) -> Rc<dyn Fn()> {
    let ctx = play_ctx.clone();
    Rc::new(move || {
        let _ = apply_mpv_pause(&ctx, true);
    })
}

#[cfg(target_os = "linux")]
fn mpris_unpause_only(play_ctx: &PlayToggleCtx) -> Rc<dyn Fn()> {
    let ctx = play_ctx.clone();
    Rc::new(move || {
        let _ = apply_mpv_pause(&ctx, false);
    })
}

#[cfg(target_os = "linux")]
fn mpris_stop(play_ctx: &PlayToggleCtx) -> Rc<dyn Fn()> {
    let ctx = play_ctx.clone();
    Rc::new(move || media_stop(&ctx))
}

#[cfg(target_os = "linux")]
fn start_linux_mpris(
    ctx: MprisLinuxWireCtx<'_>,
    seek_abs: crate::mpris::MpvSeekAbs,
    prev: Rc<dyn Fn()>,
    next: Rc<dyn Fn()>,
) {
    crate::mpris::start_linux(crate::mpris::MprisStartArgs {
        app: ctx.app.clone(),
        win: ctx.win,
        mpv_bundle: Rc::clone(ctx.player),
        seek_abs,
        toggle_play_pause: mpris_toggle_play_pause(ctx.play_ctx),
        pause_only: mpris_pause_only(ctx.play_ctx),
        unpause_only: mpris_unpause_only(ctx.play_ctx),
        stop: mpris_stop(ctx.play_ctx),
        prev,
        next,
    });
}

#[cfg(target_os = "linux")]
fn wire_mpris_linux_after_seek(ctx: MprisLinuxWireCtx<'_>) {
    let seek_abs = make_mpris_seek_abs(&ctx);
    let nav = MprisSiblingNav::from_wire_ctx(&ctx);
    let do_prev = {
        let nav = nav.clone();
        move || nav.step(sibling_advance::prev_before_current, "previous")
    };
    let do_next = {
        let nav = nav.clone();
        move || nav.step(sibling_advance::next_after_eof, "next")
    };
    start_linux_mpris(ctx, seek_abs, Rc::new(do_prev), Rc::new(do_next));
}
