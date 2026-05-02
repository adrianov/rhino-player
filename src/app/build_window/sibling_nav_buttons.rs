/// Shared refs for [try_load_sibling_pick] (bottom-bar buttons + Ctrl+arrow shortcuts).
struct SiblingNavTryRefs<'a> {
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    win: &'a adw::ApplicationWindow,
    gl: &'a gtk::GLArea,
    recent: &'a gtk::ScrolledWindow,
    last_path: &'a Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: &'a Rc<dyn Fn()>,
    win_aspect: &'a Rc<Cell<Option<f64>>>,
    sibling_seof: &'a Rc<SiblingEofState>,
    on_file_loaded: &'a Rc<dyn Fn()>,
}

/// Loads another local file using the same sibling-folder ordering as EOF advance (**Previous** /
/// **Next** buttons and Ctrl+Left / Ctrl+Right shortcuts).
fn try_load_sibling_pick(
    pick: fn(&Path) -> Option<PathBuf>,
    log_tag: &'static str,
    r: &SiblingNavTryRefs<'_>,
) {
    let cur = r.last_path.borrow().clone();
    let Some(cur) = cur.filter(|c| c.is_file()) else {
        return;
    };
    let g = r.player.borrow();
    if g.is_none() {
        return;
    }
    let Some(np) = pick(&cur) else {
        return;
    };
    r.sibling_seof.done.set(false);
    drop(g);
    let o = LoadOpts::replace_media(
        Rc::clone(r.last_path),
        Some(Rc::clone(r.on_video_chrome)),
        Rc::clone(r.win_aspect),
        Some(Rc::clone(r.on_file_loaded)),
        true,
        false,
    );
    if let Err(e) = try_load(&np, r.player, r.win, r.gl, r.recent, &o) {
        eprintln!("[rhino] {log_tag}: {e}");
    }
}

/// Wires the bottom-bar **previous** / **next** buttons to the sibling-folder queue
/// (`docs/features/07-sibling-folder-queue.md`).
///
/// Both buttons share the same body — only the `pick` lookup differs
/// (`prev_before_current` vs. `next_after_eof`). Extracting one closure factory
/// removes the duplicated `glib::clone!` block that previously inlined this
/// logic inside `build_window`.
/// Owned refs for prev/next wiring ([`wire_sibling_navigation`], keyboard shortcuts, macOS media keys).
#[derive(Clone)]
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
}

impl SiblingNavCtx {
    fn try_refs(&self) -> SiblingNavTryRefs<'_> {
        SiblingNavTryRefs {
            player: &self.player,
            win: &self.win,
            gl: &self.gl,
            recent: &self.recent,
            last_path: &self.last_path,
            on_video_chrome: &self.on_video_chrome,
            win_aspect: &self.win_aspect,
            sibling_seof: &self.sibling_seof,
            on_file_loaded: &self.on_file_loaded,
        }
    }
}

fn wire_sibling_navigation(ctx: SiblingNavCtx) -> SiblingNavCtx {
    wire_sibling_nav_buttons(ctx.clone());
    ctx
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
    move |_| {
        try_load_sibling_pick(pick, label, &SiblingNavTryRefs {
            player: &p,
            win: &w,
            gl: &gl,
            recent: &rec,
            last_path: &lp,
            on_video_chrome: &ovid,
            win_aspect: &wa,
            sibling_seof: &seof,
            on_file_loaded: &ol,
        });
    }
}
