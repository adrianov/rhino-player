/// Builds the shared file-open closure used by the menu and recent cards.
struct OpenHandlerCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_start: Rc<dyn Fn()>,
    on_loaded: Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
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
                let mut o = LoadOpts::replace_media(
                    ctx.last_path.clone(),
                    Some(Rc::clone(&ctx.on_start)),
                    ctx.win_aspect.clone(),
                    Some(Rc::clone(&ctx.on_loaded)),
                    true,
                    false,
                    ctx.hdr_title_mirror.clone(),
                );
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
