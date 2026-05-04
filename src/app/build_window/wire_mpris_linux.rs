#[cfg(target_os = "linux")]
struct MprisLinuxWireCtx<'a> {
    app: &'a adw::Application,
    win: adw::ApplicationWindow,
    gl_area: gtk::GLArea,
    recent_scrl: gtk::Box,
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    play_ctx: &'a PlayToggleCtx,
    last_path: &'a Rc<RefCell<Option<PathBuf>>>,
    win_aspect: &'a Rc<Cell<Option<f64>>>,
    sibling_seof: &'a Rc<SiblingEofState>,
    reapply_60: VideoReapply60,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    on_file_loaded: &'a Rc<dyn Fn()>,
    on_video_chrome: &'a Rc<dyn Fn()>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: &'a Rc<Cell<bool>>,
}

#[cfg(target_os = "linux")]
fn wire_mpris_linux_after_seek(ctx: MprisLinuxWireCtx<'_>) {
    let MprisLinuxWireCtx {
        app,
        win,
        gl_area,
        recent_scrl,
        player,
        play_ctx,
        last_path,
        win_aspect,
        sibling_seof,
        reapply_60,
        smooth_seek_debounce,
        resume_after_seek_idle,
        on_file_loaded,
        on_video_chrome,
        hdr_title_mirror,
        playback_focus,
    } = ctx;
    let p_seek = player.clone();
    let gl_seek = gl_area.clone();
    let r_seek = reapply_60.clone();
    let deb_seek = smooth_seek_debounce.clone();
    let resume_seek = resume_after_seek_idle.clone();
    let toggle_seek = play_ctx.clone();
    let seek_abs = crate::mpris::MpvSeekAbs(Rc::new(move |secs: &str| {
        main_player_seek_keyframes(
            &SeekKeyframeParams {
                player: &p_seek,
                gl: &gl_seek,
                reapply_60: &r_seek,
                smooth_seek_debounce: &deb_seek,
                resume_after_seek_idle: &resume_seek,
                play_toggle: &toggle_seek,
            },
            SeekKeyframeKind::ScaleOrExternal,
            secs,
        );
    }));
    let do_prev = {
        let player = player.clone();
        let win = win.clone();
        let gl = gl_area.clone();
        let rec = recent_scrl.clone();
        let last_path = last_path.clone();
        let on_video_chrome = on_video_chrome.clone();
        let win_aspect = win_aspect.clone();
        let sibling_seof = sibling_seof.clone();
        let on_loaded = Rc::clone(on_file_loaded);
        let hm = hdr_title_mirror.clone();
        let pf = Rc::clone(playback_focus);
        move || {
            try_load_sibling_pick(sibling_advance::prev_before_current, "previous", &SiblingNavTryRefs {
                player: player.clone(),
                win: win.clone(),
                gl: gl.clone(),
                recent: rec.clone(),
                last_path: last_path.clone(),
                on_video_chrome: on_video_chrome.clone(),
                win_aspect: win_aspect.clone(),
                sibling_seof: sibling_seof.clone(),
                on_file_loaded: on_loaded.clone(),
                hdr_title_mirror: hm.clone(),
                playback_focus: pf.clone(),
            });
        }
    };
    let do_next = {
        let player = player.clone();
        let win = win.clone();
        let gl = gl_area.clone();
        let rec = recent_scrl.clone();
        let last_path = last_path.clone();
        let on_video_chrome = on_video_chrome.clone();
        let win_aspect = win_aspect.clone();
        let sibling_seof = sibling_seof.clone();
        let on_loaded = Rc::clone(on_file_loaded);
        let hm = hdr_title_mirror.clone();
        let pf = Rc::clone(playback_focus);
        move || {
            try_load_sibling_pick(sibling_advance::next_after_eof, "next", &SiblingNavTryRefs {
                player: player.clone(),
                win: win.clone(),
                gl: gl.clone(),
                recent: rec.clone(),
                last_path: last_path.clone(),
                on_video_chrome: on_video_chrome.clone(),
                win_aspect: win_aspect.clone(),
                sibling_seof: sibling_seof.clone(),
                on_file_loaded: on_loaded.clone(),
                hdr_title_mirror: hm.clone(),
                playback_focus: pf.clone(),
            });
        }
    };
    let toggle_pause = play_ctx.clone();
    let pause_ctx = play_ctx.clone();
    let unpause_ctx = play_ctx.clone();
    let stop_ctx = play_ctx.clone();
    crate::mpris::start_linux(crate::mpris::MprisStartArgs {
        app: app.clone(),
        win,
        mpv_bundle: player.clone(),
        seek_abs,
        toggle_play_pause: Rc::new(move || {
            let _ = toggle_play_pause(&toggle_pause);
        }),
        pause_only: Rc::new(move || {
            let _ = apply_mpv_pause(&pause_ctx, true);
        }),
        unpause_only: Rc::new(move || {
            let _ = apply_mpv_pause(&unpause_ctx, false);
        }),
        stop: Rc::new(move || {
            media_stop(&stop_ctx);
        }),
        prev: Rc::new(do_prev),
        next: Rc::new(do_next),
    });
}
