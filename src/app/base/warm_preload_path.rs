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
    if crate::video_ext::is_dvd_vob_path(&path) {
        eprintln!(
            "[rhino] warm_preload: skip dvd chapter {} ms={}",
            path.display(),
            t0.elapsed().as_millis()
        );
        return PreloadOutcome::Failed;
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
        return PreloadOutcome::Failed;
    }
    eprintln!(
        "[rhino] warm_preload: begin {} exists={}",
        path.display(),
        path.exists()
    );
    if mpv_has_open_target(&path, player) {
        eprintln!(
            "[rhino] warm_preload: ready (already open) {} ms={}",
            path.display(),
            t0.elapsed().as_millis()
        );
        return PreloadOutcome::Ready;
    }
    if let Some(b) = player.borrow().as_ref() {
        let _ = b.mpv.set_property("pause", true);
    }
    let canon = std::fs::canonicalize(&path).ok();
    *last_path.borrow_mut() = canon;
    transport_sync_warm_browse(&path);
    let o = LoadOpts {
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
    };
    let warm_hit = match load_file_into_player(&path, player, recent, &o) {
        Err(e) => {
            eprintln!(
                "[rhino] warm_preload: failed {} ms={} err={e}",
                path.display(),
                t0.elapsed().as_millis()
            );
            return PreloadOutcome::Failed;
        }
        Ok(hit) => hit,
    };
    if warm_hit {
        eprintln!(
            "[rhino] warm_preload: warm hit {} ms={}",
            path.display(),
            t0.elapsed().as_millis()
        );
        return PreloadOutcome::Ready;
    }
    warm_preload_hold_browse_pause(player, gl);
    eprintln!(
        "[rhino] warm_preload: deferred {} ms={}",
        path.display(),
        t0.elapsed().as_millis()
    );
    PreloadOutcome::Deferred
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
    match preload_continue_path(
        &path,
        &ctx.player,
        &ctx.video_pref,
        &ctx.recent,
        &ctx.gl,
        &ctx.last_path,
    ) {
        PreloadOutcome::Deferred => {
            let gen = ctx
                .player
                .borrow()
                .as_ref()
                .map(crate::mpv_embed::MpvBundle::warm_file_gen)
                .unwrap_or(0);
            ctx.gate.set_inflight_gen(gen);
            ctx.gate
                .arm_watchdog(Rc::clone(&ctx.player), gen);
            schedule_preload_pause(Rc::clone(&ctx.player), ctx.gl.clone());
            true
        }
        PreloadOutcome::Ready => {
            let player = Rc::clone(&ctx.player);
            let gl = ctx.gl.clone();
            let run = Rc::clone(ctx);
            let gate = Rc::clone(&run.gate);
            let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
                finish_warm_preload_ready_now(&player, &gl);
                let run = Rc::clone(&run);
                gate.complete(move |p| WarmPreloadCtx::run_path(&run, p));
                glib::ControlFlow::Break
            });
            false
        }
        PreloadOutcome::Failed => {
            let run = Rc::clone(ctx);
            let gate = Rc::clone(&ctx.gate);
            gate.complete(move |p| WarmPreloadCtx::run_path(&run, p));
            false
        }
    }
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

/// Immediate hover warm preload (no debounce); shared [WarmPreloadCtx] with startup preload.
pub(crate) fn warm_hover_hooks(ctx: Rc<WarmPreloadCtx>) -> recent_view::WarmHoverHooks {
    let enter_ctx = Rc::clone(&ctx);
    let enter = Rc::new(move |path: &Path| {
        schedule_warm_hover_preload(&enter_ctx, path.to_path_buf());
    });
    let player = Rc::clone(&ctx.player);
    let gl = ctx.gl.clone();
    recent_view::WarmHoverHooks {
        enter,
        leave: Rc::new(move || warm_preload_hold_browse_pause(&player, &gl)),
    }
}
