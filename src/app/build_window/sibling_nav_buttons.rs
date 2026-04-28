/// Wires the bottom-bar **previous** / **next** buttons to the sibling-folder queue
/// (`docs/features/07-sibling-folder-queue.md`).
///
/// Both buttons share the same body — only the `pick` lookup differs
/// (`prev_before_current` vs. `next_after_eof`). Extracting one closure factory
/// removes the duplicated `glib::clone!` block that previously inlined this
/// logic inside `build_window`.
struct SiblingNavCtx {
    btn_prev: gtk::Button,
    btn_next: gtk::Button,
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    sibling_seof: Rc<SiblingEofState>,
    on_file_loaded: Rc<dyn Fn()>,
    reapply_60: VideoReapply60,
}

fn wire_sibling_nav_buttons(ctx: SiblingNavCtx) {
    ctx.btn_prev.connect_clicked(make_sibling_nav_click(
        &ctx,
        sibling_advance::prev_before_current,
        "previous",
    ));
    ctx.btn_next.connect_clicked(make_sibling_nav_click(
        &ctx,
        sibling_advance::next_after_eof,
        "next",
    ));
}

fn make_sibling_nav_click(
    ctx: &SiblingNavCtx,
    pick: fn(&Path) -> Option<PathBuf>,
    label: &'static str,
) -> impl Fn(&gtk::Button) + 'static {
    let p = ctx.player.clone();
    let w = ctx.win.clone();
    let gl = ctx.gl.clone();
    let rec = ctx.recent.clone();
    let lp = ctx.last_path.clone();
    let ovid = Rc::clone(&ctx.on_video_chrome);
    let wa = Rc::clone(&ctx.win_aspect);
    let seof = Rc::clone(&ctx.sibling_seof);
    let ol = Rc::clone(&ctx.on_file_loaded);
    let r60 = ctx.reapply_60.clone();
    move |_| {
        let cur = lp.borrow().clone();
        let Some(cur) = cur.filter(|c| c.is_file()) else {
            return;
        };
        let g = p.borrow();
        if g.is_none() {
            return;
        };
        let Some(np) = pick(&cur) else {
            return;
        };
        seof.done.set(false);
        drop(g);
        let o = LoadOpts {
            record: true,
            play_on_start: true,
            last_path: Rc::clone(&lp),
            on_start: Some(Rc::clone(&ovid)),
            win_aspect: Rc::clone(&wa),
            on_loaded: Some(Rc::clone(&ol)),
            reapply_60: Some(r60.clone()),
            reset_speed_to_normal: false,
        };
        if let Err(e) = try_load(&np, &p, &w, &gl, &rec, &o) {
            eprintln!("[rhino] {label}: {e}");
        }
    }
}
