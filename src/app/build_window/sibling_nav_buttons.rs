/// Shared refs for [try_load_sibling_pick] (bottom-bar buttons + Ctrl+arrow shortcuts).
struct SiblingNavTryRefs {
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    on_video_chrome: Rc<dyn Fn()>,
    win_aspect: Rc<WinAspectCell>,
    sibling_seof: Rc<SiblingEofState>,
    on_file_loaded: Rc<dyn Fn()>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
}

/// Loads another local file using the same sibling-folder ordering as EOF advance (**Previous** /
/// **Next** buttons and Ctrl+Left / Ctrl+Right shortcuts).
fn try_load_sibling_pick(
    pick: fn(&Path) -> Option<PathBuf>,
    log_tag: &'static str,
    r: &SiblingNavTryRefs,
) {
    let cur = r.last_path.borrow().clone();
    let Some(cur) = cur.filter(|c| c.is_file()) else {
        return;
    };
    if crate::app::browse_overlay_active(&r.recent) {
        return;
    }
    let g = r.player.borrow();
    if g.is_none() {
        return;
    }
    let Some(np) = pick(&cur) else {
        return;
    };
    r.sibling_seof.done.set(false);
    drop(g);
    let mut o = LoadOpts::replace_media(ReplaceMediaBundled {
        video_pref: Rc::clone(&r.video_pref),
        last_path: Rc::clone(&r.last_path),
        on_start: Some(Rc::clone(&r.on_video_chrome)),
        win_aspect: Rc::clone(&r.win_aspect),
        on_loaded: Some(Rc::clone(&r.on_file_loaded)),
        play_on_start: true,
        reset_speed_to_normal: false,
        hdr_title_mirror: r.hdr_title_mirror.clone(),
    });
    o.playback_focus = Some(Rc::clone(&r.playback_focus));
    if let Err(e) = try_load(&np, &r.player, &r.win, &r.gl, &r.recent, &o) {
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
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    on_video_chrome: Rc<dyn Fn()>,
    win_aspect: Rc<WinAspectCell>,
    sibling_seof: Rc<SiblingEofState>,
    on_file_loaded: Rc<dyn Fn()>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
}

impl SiblingNavCtx {
    fn try_refs(&self) -> SiblingNavTryRefs {
        SiblingNavTryRefs {
            player: self.player.clone(),
            win: self.win.clone(),
            gl: self.gl.clone(),
            recent: self.recent.clone(),
            last_path: self.last_path.clone(),
            video_pref: Rc::clone(&self.video_pref),
            on_video_chrome: self.on_video_chrome.clone(),
            win_aspect: self.win_aspect.clone(),
            sibling_seof: self.sibling_seof.clone(),
            on_file_loaded: self.on_file_loaded.clone(),
            hdr_title_mirror: self.hdr_title_mirror.clone(),
            playback_focus: Rc::clone(&self.playback_focus),
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
    let r = ctx.try_refs();
    move |_| try_load_sibling_pick(pick, label, &r)
}
