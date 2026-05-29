/// Builds the shared file-open closure used by the menu and recent cards.
struct OpenHandlerCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    on_start: Rc<dyn Fn()>,
    on_loaded: Rc<dyn Fn()>,
    win_aspect: Rc<WinAspectCell>,
    sub_menu: gtk::MenuButton,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
}

fn make_on_open_handler(ctx: OpenHandlerCtx) -> RcPathFn {
    Rc::new(move |path: &Path| {
        eprintln!("[rhino] on_open from recent/menu: {}", path.display());
        let loaded = try_load(
            path,
            &ctx.player,
            &ctx.win,
            &ctx.gl,
            &ctx.recent,
            &{
                let mut o = LoadOpts::replace_media(ReplaceMediaBundled {
                    video_pref: Rc::clone(&ctx.video_pref),
                    last_path: ctx.last_path.clone(),
                    on_start: Some(Rc::clone(&ctx.on_start)),
                    win_aspect: ctx.win_aspect.clone(),
                    on_loaded: Some(Rc::clone(&ctx.on_loaded)),
                    play_on_start: true,
                    reset_speed_to_normal: false,
                    hdr_title_mirror: ctx.hdr_title_mirror.clone(),
                });
                o.playback_focus = Some(Rc::clone(&ctx.playback_focus));
                o
            },
        );
        match loaded {
            Ok(()) => schedule_sub_button_scan(ctx.player.clone(), ctx.sub_menu.clone()),
            Err(e) => eprintln!("[rhino] on_open: try_load error: {e}"),
        }
    })
}
