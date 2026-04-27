/// Builds the shared file-open closure used by the menu and recent cards.
struct OpenHandlerCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_start: Rc<dyn Fn()>,
    on_loaded: Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    reapply_60: VideoReapply60,
    sub_menu: gtk::MenuButton,
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
            &LoadOpts {
                record: true,
                play_on_start: true,
                last_path: ctx.last_path.clone(),
                on_start: Some(Rc::clone(&ctx.on_start)),
                win_aspect: ctx.win_aspect.clone(),
                on_loaded: Some(Rc::clone(&ctx.on_loaded)),
                reapply_60: Some(ctx.reapply_60.clone()),
            },
        );
        match loaded {
            Ok(()) => schedule_sub_button_scan(ctx.player.clone(), ctx.sub_menu.clone()),
            Err(e) => eprintln!("[rhino] on_open: try_load error: {e}"),
        }
    })
}
