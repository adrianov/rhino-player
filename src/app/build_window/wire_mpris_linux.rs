#[cfg(target_os = "linux")]
fn wire_mpris_linux_after_seek(
    app: &adw::Application,
    win: adw::ApplicationWindow,
    gl_area: gtk::GLArea,
    recent_scrl: gtk::ScrolledWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    play_ctx: &PlayToggleCtx,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
    win_aspect: &Rc<Cell<Option<f64>>>,
    sibling_seof: &Rc<SiblingEofState>,
    reapply_60: VideoReapply60,
    on_file_loaded: &Rc<dyn Fn()>,
    on_video_chrome: &Rc<dyn Fn()>,
) {
    let p_seek = player.clone();
    let gl_seek = gl_area.clone();
    let r_seek = reapply_60.clone();
    let seek_abs = crate::mpris::MpvSeekAbs(Rc::new(move |secs: &str| {
        main_player_seek_keyframes(&p_seek, &gl_seek, &r_seek, secs);
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
        move || {
            try_load_sibling_pick(sibling_advance::prev_before_current, "previous", &SiblingNavTryRefs {
                player: &player,
                win: &win,
                gl: &gl,
                recent: &rec,
                last_path: &last_path,
                on_video_chrome: &on_video_chrome,
                win_aspect: &win_aspect,
                sibling_seof: &sibling_seof,
                on_file_loaded: &on_loaded,
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
        move || {
            try_load_sibling_pick(sibling_advance::next_after_eof, "next", &SiblingNavTryRefs {
                player: &player,
                win: &win,
                gl: &gl,
                recent: &rec,
                last_path: &last_path,
                on_video_chrome: &on_video_chrome,
                win_aspect: &win_aspect,
                sibling_seof: &sibling_seof,
                on_file_loaded: &on_loaded,
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
