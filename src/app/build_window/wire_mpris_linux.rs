        let p_seek = player.clone();
        let gl_seek = w.gl_area.clone();
        let r_seek = reapply_60.clone();
        let seek_abs = crate::mpris::MpvSeekAbs(Rc::new(move |secs: &str| {
            main_player_seek_keyframes(&p_seek, &gl_seek, &r_seek, secs);
        }));
        let do_prev = {
            let player = player.clone();
            let win = w.win.clone();
            let gl = w.gl_area.clone();
            let rec = w.recent_scrl.clone();
            let last_path = last_path.clone();
            let on_video_chrome = on_video_chrome.clone();
            let win_aspect = win_aspect.clone();
            let sibling_seof = sibling_seof.clone();
            let on_loaded = Rc::clone(&on_file_loaded);
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
            let win = w.win.clone();
            let gl = w.gl_area.clone();
            let rec = w.recent_scrl.clone();
            let last_path = last_path.clone();
            let on_video_chrome = on_video_chrome.clone();
            let win_aspect = win_aspect.clone();
            let sibling_seof = sibling_seof.clone();
            let on_loaded = Rc::clone(&on_file_loaded);
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
        crate::mpris::start_linux(crate::mpris::MprisStartArgs {
            app: app.clone(),
            win: w.win.clone(),
            mpv_bundle: player.clone(),
            seek_abs,
            toggle_play_pause: Rc::new({
                let c = play_ctx.clone();
                move || {
                    let _ = toggle_play_pause(&c);
                }
            }),
            pause_only: Rc::new({
                let c = play_ctx.clone();
                move || {
                    let _ = apply_mpv_pause(&c, true);
                }
            }),
            unpause_only: Rc::new({
                let c = play_ctx.clone();
                move || {
                    let _ = apply_mpv_pause(&c, false);
                }
            }),
            stop: Rc::new({
                let c = play_ctx.clone();
                move || {
                    media_stop(&c);
                }
            }),
            prev: Rc::new(do_prev),
            next: Rc::new(do_next),
        });
