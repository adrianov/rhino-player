// Sibling EOF advance execution: finished-target resolution, load targets, and the
// `try_load` that advances to the next file in folder order (included by [eof_advance_nav]).

/// Resolve the finished title (mpv path, else last open path); marks the one-shot EOF guard
/// consumed when neither is available.
fn eof_finished_target(
    pl: &MpvBundle,
    seof: &SiblingEofState,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> Option<PathBuf> {
    let finished = local_file_from_mpv(&pl.mpv).or_else(|| last_path.borrow().clone());
    if finished.is_none() {
        seof.done.set(true);
    }
    finished
}

/// Player/window handles threaded through the EOF advance load path.
struct LoadTargets<'a> {
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    win: &'a adw::ApplicationWindow,
    gl: &'a gtk::GLArea,
    recent: &'a gtk::Box,
}

/// Borrowed shared state for the sibling reload (cloned only where the opts builder needs it).
struct SiblingReloadRefs<'a> {
    seof: &'a SiblingEofState,
    last_path: &'a Rc<RefCell<Option<PathBuf>>>,
    video_pref: &'a Rc<RefCell<db::VideoPrefs>>,
    on_start: &'a Rc<dyn Fn()>,
    win_aspect: &'a Rc<WinAspectCell>,
    on_open_fail: &'a Rc<dyn Fn(String)>,
}

/// Load the next file in folder order, or drop the finished title from the continue list and
/// DB when no follow-up file exists.
fn advance_to_next_sibling(
    finished: PathBuf,
    t: LoadTargets<'_>,
    r: SiblingReloadRefs<'_>,
    on_loaded: &Option<Rc<dyn Fn()>>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
) {
    r.seof.done.set(true);
    let Some(np) = sibling_advance::next_after_eof(&finished) else {
        // [try_load] only runs on a path change; with no follow-up file, EOF still left the
        // title in the continue list and DB — drop both here.
        remove_continue_entry(&finished);
        return;
    };
    if crate::video_ext::paths_same_file(&np, &finished) {
        return;
    }
    let o = sibling_reload_opts(&r, on_loaded, hdr_title_mirror, playback_focus);
    if let Err(e) = try_load(&np, t.player, t.win, t.gl, t.recent, &o) {
        eprintln!("[rhino] sibling advance: {e}");
        r.seof.done.set(false);
    }
}

/// `replace_media` options for the sibling auto-advance (play immediately, speed reset).
fn sibling_reload_opts(
    r: &SiblingReloadRefs<'_>,
    on_loaded: &Option<Rc<dyn Fn()>>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
) -> LoadOpts {
    let mut o = LoadOpts::replace_media(ReplaceMediaBundled {
        video_pref: Rc::clone(r.video_pref),
        last_path: Rc::clone(r.last_path),
        on_start: Some(Rc::clone(r.on_start)),
        win_aspect: Rc::clone(r.win_aspect),
        on_loaded: on_loaded.as_ref().map(Rc::clone),
        play_on_start: true,
        reset_speed_to_normal: true,
        hdr_title_mirror,
    });
    o.playback_focus = Some(playback_focus);
    o.on_open_fail = Some(Rc::clone(r.on_open_fail));
    o
}
