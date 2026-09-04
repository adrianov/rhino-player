/// Early-exit checks for a hover/startup warm load. Returns the terminal outcome when `path`
/// cannot start a warm load (DVD chapter, missing file, already open); `None` to continue.
fn warm_preload_early_outcome(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &impl IsA<gtk::Widget>,
    t0: std::time::Instant,
) -> Option<PreloadOutcome> {
    if crate::video_ext::is_dvd_vob_path(path) {
        eprintln!(
            "[rhino] warm_preload: skip dvd chapter {} ms={}",
            path.display(),
            t0.elapsed().as_millis()
        );
        return Some(PreloadOutcome::Failed);
    }
    if !recent.is_visible() || !path.is_file() || player.borrow().is_none() {
        eprintln!(
            "[rhino] warm_preload: skip {} ms={} (recent={} file={} player={})",
            path.display(),
            t0.elapsed().as_millis(),
            recent.is_visible(),
            path.is_file(),
            player.borrow().is_some()
        );
        return Some(PreloadOutcome::Failed);
    }
    eprintln!(
        "[rhino] warm_preload: begin {} exists={}",
        path.display(),
        path.exists()
    );
    if mpv_has_open_target(path, player) {
        eprintln!(
            "[rhino] warm_preload: ready (already open) {} ms={}",
            path.display(),
            t0.elapsed().as_millis()
        );
        return Some(PreloadOutcome::Ready);
    }
    None
}

/// Load options for a background warm preload: no history record, no autoplay, no callbacks.
fn warm_load_opts(
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> LoadOpts {
    LoadOpts {
        video_pref: Rc::clone(video_pref),
        record: false,
        play_on_start: false,
        last_path: Rc::clone(last_path),
        on_start: None,
        win_aspect: Rc::new(Cell::new(None)),
        on_loaded: None,
        reset_speed_to_normal: false,
        hdr_title_mirror: None,
        playback_focus: None,
        warm_preload: true,
        on_open_fail: None,
    }
}

/// Pause playback, remember the canonical path, and start the async warm `loadfile`.
fn dispatch_warm_load(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    recent: &impl IsA<gtk::Widget>,
    gl: &gtk::GLArea,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
    t0: std::time::Instant,
) -> PreloadOutcome {
    if let Some(b) = player.borrow().as_ref() {
        let _ = b.mpv.set_property("pause", true);
    }
    *last_path.borrow_mut() = std::fs::canonicalize(path).ok();
    transport_sync_warm_browse(path);
    match load_file_into_player(path, player, recent, &warm_load_opts(video_pref, last_path)) {
        Err(e) => {
            eprintln!(
                "[rhino] warm_preload: failed {} ms={} err={e}",
                path.display(),
                t0.elapsed().as_millis()
            );
            PreloadOutcome::Failed
        }
        Ok(true) => {
            eprintln!(
                "[rhino] warm_preload: warm hit {} ms={}",
                path.display(),
                t0.elapsed().as_millis()
            );
            PreloadOutcome::Ready
        }
        Ok(false) => {
            warm_preload_hold_browse_pause(player, gl);
            eprintln!(
                "[rhino] warm_preload: deferred {} ms={}",
                path.display(),
                t0.elapsed().as_millis()
            );
            PreloadOutcome::Deferred
        }
    }
}

fn preload_continue_path(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    recent: &impl IsA<gtk::Widget>,
    gl: &gtk::GLArea,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> PreloadOutcome {
    let t0 = std::time::Instant::now();
    let path = crate::video_ext::resolve_open_media_path(path);
    if let Some(outcome) = warm_preload_early_outcome(&path, player, recent, t0) {
        return outcome;
    }
    dispatch_warm_load(&path, player, video_pref, recent, gl, last_path, t0)
}

fn preload_first_continue(ctx: &Rc<WarmPreloadCtx>) -> bool {
    if !ctx.recent.is_visible() || ctx.last_path.borrow().is_some() {
        return false;
    }
    let path = match history::load().into_iter().next() {
        Some(p) => p,
        None => return false,
    };
    eprintln!(
        "[rhino] warm_preload: first continue card {}",
        path.display()
    );
    if !ctx.gate.try_begin() {
        ctx.gate.queue(path);
        return false;
    }
    settle_preload_outcome(
        ctx,
        preload_continue_path(
            &path,
            &ctx.player,
            &ctx.video_pref,
            &ctx.recent,
            &ctx.gl,
            &ctx.last_path,
        ),
    )
}

/// Warm-preload the first continue entry after transport observers are installed.
fn run_continue_warm_preload(ctx: &Rc<WarmPreloadCtx>, skip_followups: bool) {
    if !preload_first_continue(ctx) {
        return;
    }
    if skip_followups {
        ctx.gate.complete(move |_| ());
    }
}

fn schedule_preload_pause(player: Rc<RefCell<Option<MpvBundle>>>, gl: gtk::GLArea) {
    let _ = glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        warm_preload_hold_browse_pause(&player, &gl);
        glib::ControlFlow::Break
    });
}

/// Continue-card hover: seek bar from stored length/resume only (no `loadfile`).
pub(crate) fn warm_hover_hooks(ctx: Rc<WarmPreloadCtx>) -> recent_view::WarmHoverHooks {
    let player = Rc::clone(&ctx.player);
    let gl = ctx.gl.clone();
    recent_view::WarmHoverHooks {
        enter: Rc::new(|path: &Path| {
            transport_sync_warm_browse(path);
        }),
        leave: Rc::new(move || warm_preload_hold_browse_pause(&player, &gl)),
    }
}
