/// Play toggle, Open handler, drop targets, and sibling Prev/Next — one open path.
struct MediaOpenWire {
    play_ctx: PlayToggleCtx,
    on_open: RcPathFn,
}

struct MediaOpenParts<'a> {
    app: &'a adw::Application,
    w: &'a WindowWidgets,
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &'a Rc<RefCell<db::VideoPrefs>>,
    last_path: &'a Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    win_aspect: &'a Rc<WinAspectCell>,
    playback_focus: &'a Rc<Cell<bool>>,
    sibling_seof: &'a Rc<SiblingEofState>,
    on_open_fail: Rc<dyn Fn(String)>,
    on_open_slot: &'a Rc<RefCell<Option<RcPathFn>>>,
}

impl MediaOpenWire {
    fn attach(p: MediaOpenParts<'_>) -> Self {
        let play_ctx = PlayToggleCtx {
            app: p.app.clone(),
            player: p.player.clone(),
            video_pref: Rc::clone(p.video_pref),
            win: p.w.win.clone(),
            video_handle: p.w.video_handle.clone(),
            gl: p.w.gl_area.clone(),
            recent: p.w.recent_scrl.clone(),
            last_path: p.last_path.clone(),
            on_video_chrome: Rc::clone(&p.on_video_chrome),
            on_file_loaded: Rc::clone(&p.on_file_loaded),
            win_aspect: p.win_aspect.clone(),
            sub_menu: Some(p.w.sub_menu.clone()),
            play_pause: p.w.play_pause.clone(),
            hdr_title_mirror: p.w.hdr_title_mirror.clone(),
            playback_focus: Rc::clone(p.playback_focus),
            incomplete_hold: Rc::clone(&p.sibling_seof.incomplete_hold),
        };
        wire_play_toggles(&p.w.play_pause, play_ctx.clone());
        let on_open = make_on_open_handler(OpenHandlerCtx {
            player: p.player.clone(),
            win: p.w.win.clone(),
            gl: p.w.gl_area.clone(),
            recent: p.w.recent_scrl.clone(),
            last_path: p.last_path.clone(),
            video_pref: Rc::clone(p.video_pref),
            on_start: Rc::clone(&p.on_video_chrome),
            on_loaded: Rc::clone(&p.on_file_loaded),
            win_aspect: Rc::clone(p.win_aspect),
            sub_menu: p.w.sub_menu.clone(),
            hdr_title_mirror: p.w.hdr_title_mirror.clone(),
            playback_focus: Rc::clone(p.playback_focus),
            on_open_fail: Rc::clone(&p.on_open_fail),
        });
        *p.on_open_slot.borrow_mut() = Some(on_open.clone());
        wire_window_drop_targets(&p.w.win, p.player, &p.w.sub_menu, &on_open);
        wire_sibling_navigation(SiblingNavCtx {
            btn_prev: p.w.sibling_nav.prev_btn.clone(),
            btn_next: p.w.sibling_nav.next_btn.clone(),
            win: p.w.win.clone(),
            gl: p.w.gl_area.clone(),
            recent: p.w.recent_scrl.clone(),
            player: p.player.clone(),
            last_path: p.last_path.clone(),
            video_pref: Rc::clone(p.video_pref),
            on_video_chrome: Rc::clone(&p.on_video_chrome),
            win_aspect: p.win_aspect.clone(),
            sibling_seof: p.sibling_seof.clone(),
            on_file_loaded: Rc::clone(&p.on_file_loaded),
            hdr_title_mirror: p.w.hdr_title_mirror.clone(),
            playback_focus: Rc::clone(p.playback_focus),
            on_open_fail: Rc::clone(&p.on_open_fail),
        });
        Self { play_ctx, on_open }
    }
}
