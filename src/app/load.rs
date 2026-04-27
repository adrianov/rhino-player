/// [video_pref::apply_mpv_video] after [loadfile] so the VapourSynth filter attaches when [path] is valid.
#[derive(Clone)]
struct VideoReapply60 {
    vp: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
}

/// Options for [try_load] (keeps the arity clippy limit without `allow`).
struct LoadOpts {
    record: bool,
    play_on_start: bool,
    /// Filled on success so [maybe_advance_sibling_on_eof] can resolve a path if mpv clears it at idle EOF.
    last_path: Rc<RefCell<Option<PathBuf>>>,
    /// Reveal chrome and (re)start 3s auto-hide; `None` for tests or callers without UI bundle.
    on_start: Option<Rc<dyn Fn()>>,
    /// `Some(w/h)` for [sync_window_aspect_from_mpv] / [apply_window_video_aspect]; cleared with no video.
    win_aspect: Rc<Cell<Option<f64>>>,
    /// Fuzzy subtitle auto-pick + hook after a successful `loadfile`.
    on_loaded: Option<Rc<dyn Fn()>>,
    reapply_60: Option<VideoReapply60>,
}

/// Load a file, hide the recent grid overlay, show video; [LoadOpts::record] appends to recent history.
/// [play_on_start]: clear `pause` so playback runs (watch_later can restore a paused file after load; a
/// short delayed [set_property] catches that). **false** for CLI open-on-launch to respect saved state.
fn try_load(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
) -> Result<(), String> {
    let play_on_start = o.play_on_start;
    let record = o.record;
    eprintln!(
        "[rhino] try_load: path={} exists={} record={} player_ready={} play={}",
        path.display(),
        path.exists(),
        record,
        player.borrow().is_some(),
        play_on_start
    );
    let mut warm_hit = false;
    {
        let mut g = player.borrow_mut();
        let b = g.as_mut().ok_or("Player not ready. Wait for GL init.")?;
        let prev = local_file_from_mpv(&b.mpv).or_else(|| o.last_path.borrow().clone());
        let already_loaded =
            recent_layer.is_visible() && prev.as_ref().is_some_and(|p| same_open_target(p, path));
        if already_loaded {
            warm_hit = true;
            eprintln!("[rhino] try_load: warm preload hit");
        } else {
            let clear_outgoing_resume =
                is_done_enough_to_drop_continue(&b.mpv) && local_file_from_mpv(&b.mpv).is_some();
            let drop_from_history = prev.as_ref().is_some_and(|p| {
                !same_open_target(p, path) && is_done_enough_to_drop_continue(&b.mpv)
            });
            if let Err(e) = b.load_file_path(path, clear_outgoing_resume) {
                eprintln!("[rhino] try_load: loadfile failed: {e}");
                return Err(e);
            }
            eprintln!("[rhino] try_load: loadfile ok");
            if drop_from_history {
                if let Some(p) = prev {
                    remove_continue_entry(&p);
                }
            }
        }
    }
    if !warm_hit {
        if let Some(r) = o.reapply_60.as_ref() {
            let p = Rc::clone(player);
            let r0 = r.clone();
            let _ = glib::idle_add_local_once(move || {
                if let Some(b) = p.borrow().as_ref() {
                    let a = {
                        let mut g = r0.vp.borrow_mut();
                        video_pref::apply_mpv_video(&b.mpv, &mut g, None)
                    };
                    if a.smooth_auto_off {
                        sync_smooth_60_to_off(&r0.app);
                        if !can_find_mvtools(&r0.vp.borrow()) {
                            show_smooth_setup_dialog(&r0.app);
                        }
                    }
                }
                let p2 = Rc::clone(&p);
                let r1 = r0.clone();
                let _ = glib::idle_add_local_once(move || {
                    if let Some(b) = p2.borrow().as_ref() {
                        let off = {
                            let mut g = r1.vp.borrow_mut();
                            video_pref::reapply_60_if_still_missing(&b.mpv, &mut g)
                        };
                        if off {
                            sync_smooth_60_to_off(&r1.app);
                            if !can_find_mvtools(&r1.vp.borrow()) {
                                show_smooth_setup_dialog(&r1.app);
                            }
                        }
                    }
                });
            });
        }
    }
    *o.last_path.borrow_mut() = std::fs::canonicalize(path).ok();
    if record {
        history::record(path);
    }
    let t = title_for_open_path(path);
    win.set_title(Some(t.as_str()));
    let delayed_warm = warm_hit && play_on_start;
    if !delayed_warm {
        recent_layer.set_visible(false);
        // on_start may call apply_chrome, which borrow()s the player; drop the try_load borrow_mut first.
        if let Some(f) = o.on_start.as_ref() {
            f();
        }
    }
    gl.queue_render();
    if play_on_start {
        // Raise the window if the app was in the background (another app focused / minimized).
        if delayed_warm {
            if let Some(b) = player.borrow().as_ref() {
                resync_warm_continue(&b.mpv);
            }
            let recent = recent_layer.as_ref().clone();
            let win2 = win.clone();
            let gl2 = gl.clone();
            let player2 = player.clone();
            let on_start = o.on_start.clone();
            let _ =
                glib::timeout_add_local(Duration::from_millis(WARM_REVEAL_DELAY_MS), move || {
                    recent.set_visible(false);
                    if let Some(f) = on_start.as_ref() {
                        f();
                    }
                    win2.present();
                    if let Some(b) = player2.borrow().as_ref() {
                        let _ = b.mpv.set_property("pause", false);
                    }
                    gl2.queue_render();
                    glib::ControlFlow::Break
                });
        } else {
            win.present();
            if let Some(b) = player.borrow().as_ref() {
                let _ = b.mpv.set_property("pause", false);
            }
            let p2 = Rc::clone(player);
            let _ =
                glib::source::timeout_add_local(std::time::Duration::from_millis(100), move || {
                    if let Some(b) = p2.borrow().as_ref() {
                        let _ = b.mpv.set_property("pause", false);
                    }
                    glib::ControlFlow::Break
                });
        }
    }
    if let Some(b) = player.borrow().as_ref() {
        sync_window_aspect_from_mpv(&b.mpv, o.win_aspect.as_ref());
    }
    schedule_window_fit_h_video(Rc::clone(player), win.clone());
    if !warm_hit {
        if let Some(f) = o.on_loaded.clone() {
            glib::source::idle_add_local_once(move || f());
        }
    }
    Ok(())
}

fn save_mpv_audio(mpv: &Mpv) {
    let vol = mpv.get_property::<f64>("volume").unwrap_or(100.0);
    let muted = mpv.get_property::<bool>("mute").unwrap_or(false);
    db::save_audio(vol, muted);
}

fn save_mpv_state(mpv: &Mpv, sub: &RefCell<db::SubPrefs>) {
    save_mpv_audio(mpv);
    let mut p = sub.borrow_mut();
    if let Ok(sc) = mpv.get_property::<f64>("sub-scale") {
        if sc.is_finite() {
            p.scale = sc;
        }
    }
    db::save_sub(&p);
}

fn vol_icon(muted: bool, vol: f64) -> &'static str {
    if muted || vol < 0.5 {
        "audio-volume-muted-symbolic"
    } else if vol < 33.0 {
        "audio-volume-low-symbolic"
    } else if vol < 66.0 {
        "audio-volume-medium-symbolic"
    } else {
        "audio-volume-high-symbolic"
    }
}

/// Header sound popover: mute icon only (fader next to it shows level).
fn vol_mute_pop_icon(muted: bool) -> &'static str {
    if muted {
        "audio-volume-muted-symbolic"
    } else {
        "audio-volume-high-symbolic"
    }
}

const SIBLING_END_SLACK_SEC: f64 = 1.75;
const SIBLING_POS_STALL_TICKS: u8 = 3;
const SIBLING_POS_EPS: f64 = 0.04;

/// State for `maybe_advance_sibling_on_eof`: one-shot flag and tail stall detection.
struct SiblingEofState {
    done: Cell<bool>,
    stall: Cell<(f64, u8)>,
    /// Last canonical path for which `nav_sensitivity` was computed; avoids `prev` / `next` directory walks every 200ms.
    nav_key: RefCell<Option<PathBuf>>,
    nav_can_prev: Cell<bool>,
    nav_can_next: Cell<bool>,
}

impl SiblingEofState {
    /// Prev/next button sensitivity for `cur`. Reuses cached fs work while the file path is unchanged.
    fn nav_sensitivity(&self, cur: &Path) -> (bool, bool) {
        if !cur.is_file() {
            *self.nav_key.borrow_mut() = None;
            return (false, false);
        }
        let can = match std::fs::canonicalize(cur) {
            Ok(p) => p,
            Err(_) => {
                *self.nav_key.borrow_mut() = None;
                return (false, false);
            }
        };
        {
            let k = self.nav_key.borrow();
            if k.as_ref() == Some(&can) {
                return (self.nav_can_prev.get(), self.nav_can_next.get());
            }
        }
        let cp = sibling_advance::prev_before_current(cur).is_some();
        let cn = sibling_advance::next_after_eof(cur).is_some();
        *self.nav_key.borrow_mut() = Some(can);
        self.nav_can_prev.set(cp);
        self.nav_can_next.set(cn);
        (cp, cn)
    }

    fn clear_nav_sensitivity(&self) {
        *self.nav_key.borrow_mut() = None;
    }
}

/// `eof-reached` is the usual “finished” signal, but with `keep-open` and the GL render path it can stay
/// false while `time-pos` sits just short of `duration` (e.g. one second left) so nothing advances. We also
/// treat as natural end: **unpaused**, within `SIBLING_END_SLACK_SEC` of the end, and the same `time-pos` for
/// `SIBLING_POS_STALL_TICKS` consecutive poll periods (~200 ms each) — playback stuck in the tail.
/// `sibling_eof_done` still allows a single `try_load` per logical end. Clears when not at an end state.
#[allow(clippy::too_many_arguments)]
fn maybe_advance_sibling_on_eof(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent: &gtk::ScrolledWindow,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
    seof: &SiblingEofState,
    exit_after_current: &Rc<Cell<bool>>,
    app: &adw::Application,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
    idle_inhib: &Rc<RefCell<Option<u32>>>,
    on_start: &Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    on_loaded: Option<Rc<dyn Fn()>>,
    reapply: &VideoReapply60,
) {
    let g = match player.try_borrow() {
        Ok(b) => b,
        Err(_) => return,
    };
    let Some(pl) = g.as_ref() else {
        return;
    };
    let eof = pl.mpv.get_property::<bool>("eof-reached").unwrap_or(false);
    let pos = pl.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    let dur = pl.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let paused = pl.mpv.get_property::<bool>("pause").unwrap_or(true);
    let rem = if dur > 0.0 && pos.is_finite() {
        dur - pos
    } else {
        f64::INFINITY
    };
    let in_slack = dur > 0.0 && rem <= SIBLING_END_SLACK_SEC;
    if paused || !in_slack || eof {
        seof.stall.set((0.0, 0));
    } else {
        let (lp, n) = seof.stall.get();
        if (pos - lp).abs() < SIBLING_POS_EPS {
            seof.stall.set((lp, n.saturating_add(1).min(250)));
        } else {
            seof.stall.set((pos, 0));
        }
    }
    let stalled = in_slack && !paused && !eof && seof.stall.get().1 >= SIBLING_POS_STALL_TICKS;
    let at_end = eof || stalled;
    if !at_end {
        seof.done.set(false);
        return;
    }
    if seof.done.get() {
        return;
    }
    if exit_after_current.get() {
        seof.done.set(true);
        seof.stall.set((0.0, 0));
        drop(g);
        schedule_quit_persist(app, win, player, sub_pref, idle_inhib);
        return;
    }
    let finished = local_file_from_mpv(&pl.mpv).or_else(|| last_path.borrow().clone());
    let Some(finished) = finished else {
        seof.done.set(true);
        seof.stall.set((0.0, 0));
        return;
    };
    let next = sibling_advance::next_after_eof(&finished);
    let no_sibling = next.is_none();
    drop(g);
    seof.done.set(true);
    if let Some(np) = next {
        let o = LoadOpts {
            record: true,
            play_on_start: true,
            last_path: Rc::clone(last_path),
            on_start: Some(Rc::clone(on_start)),
            win_aspect: Rc::clone(&win_aspect),
            on_loaded: on_loaded.as_ref().map(Rc::clone),
            reapply_60: Some(reapply.clone()),
        };
        if let Err(e) = try_load(&np, player, win, gl, recent, &o) {
            eprintln!("[rhino] sibling advance: {e}");
            seof.done.set(false);
            seof.stall.set((0.0, 0));
        }
    } else if no_sibling {
        // [try_load] only runs on a path change; with no follow-up file, EOF still left the
        // title in continue + watch_later — drop both here.
        remove_continue_entry(&finished);
    }
}

/// Bottom-bar **Previous** / **Next** tooltips: the **file name** of the target in folder/sibling
/// order; [can] is from [SiblingEofState::nav_sensitivity].
fn sibling_bar_tooltip(is_prev: bool, can: bool, cur: Option<&Path>) -> String {
    if !can {
        return if is_prev {
            "No previous file in folder order".to_string()
        } else {
            "No next file in folder order".to_string()
        };
    }
    let Some(c) = cur else {
        return if is_prev {
            "Open previous in folder order".to_string()
        } else {
            "Open next in folder order".to_string()
        };
    };
    let t = if is_prev {
        sibling_advance::prev_before_current(c)
    } else {
        sibling_advance::next_after_eof(c)
    };
    let Some(t) = t else {
        // Rare if [can] and [cur] match [nav_sensitivity]; keep a neutral line if paths diverge.
        return if is_prev {
            "Previous in folder order".to_string()
        } else {
            "Next in folder order".to_string()
        };
    };
    // File name only (non-utf8: lossy); icon shows previous vs next.
    t.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| t.to_string_lossy().into_owned())
}

fn nudge_mpv_volume(mpv: &Mpv, delta: f64) {
    let max = mpv
        .get_property::<f64>("volume-max")
        .unwrap_or(100.0)
        .max(1.0);
    let cur = mpv.get_property::<f64>("volume").unwrap_or(0.0);
    let nv = (cur + delta).clamp(0.0, max);
    let _ = mpv.set_property("volume", nv);
    if nv > 0.5 {
        let _ = mpv.set_property("mute", false);
    }
}

/// Rebuild the continue row from [history] after a remove or undo.
fn reflow_continue_cards(
    row: &gtk::Box,
    recent: &gtk::ScrolledWindow,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    rbf: &Rc<RefCell<Option<Rc<RecentContext>>>>,
) {
    let r: Vec<PathBuf> = history::load().into_iter().take(5).collect();
    if r.is_empty() {
        recent.set_visible(false);
        return;
    }
    recent.set_visible(true);
    let v: Vec<CardData> = card_data_list(&r);
    recent_view::fill_row(row, v, on_open.clone(), on_remove.clone(), on_trash.clone());
    let n = recent_view::ensure_recent_backfill(rbf, row, on_open, on_remove, on_trash);
    recent_view::schedule_thumb_backfill(n, r);
}

fn cancel_undo_timer(src: &RefCell<Option<glib::source::SourceId>>) {
    if let Some(id) = src.borrow_mut().take() {
        id.remove();
    }
}

/// LIFO stack: label shows the file that **Undo** will restore; dismiss / timeout discards that undo target only.
fn sync_undo_bar(
    label: &gtk::Label,
    btn: &gtk::Button,
    shell: &gtk::Box,
    stack: &RefCell<Vec<ContinueBarUndo>>,
) {
    let n = stack.borrow().len();
    shell.set_visible(n > 0);
    if n == 0 {
        label.set_label("");
        btn.set_tooltip_text(None);
        return;
    }
    match n {
        1 => btn.set_tooltip_text(Some(
            "Undo: put the file back on the list with prior resume/cache, or restore from trash when the last action was trash.",
        )),
        n => {
            let s = format!(
                "Restores the most recent action. {n} step(s) on the stack (one per click, newest first)."
            );
            btn.set_tooltip_text(Some(s.as_str()));
        }
    }
    if let Some(p) = stack.borrow().last() {
        let (name, tail) = match p {
            ContinueBarUndo::ListRemove(u) => (
                u.path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file"),
                "removed from continue list",
            ),
            ContinueBarUndo::Trash { snap, .. } => (
                snap.path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file"),
                "moved to trash",
            ),
        };
        let line = format!("\u{201c}{name}\u{201d} {tail}");
        label.set_label(&line);
    }
}

fn rearm_undo_dismiss(
    do_commit: &Rc<dyn Fn() + 'static>,
    undo_source: &RefCell<Option<glib::source::SourceId>>,
) {
    cancel_undo_timer(undo_source);
    let c = do_commit.clone();
    *undo_source.borrow_mut() = Some(glib::timeout_add_seconds_local(10, move || {
        c();
        glib::ControlFlow::Break
    }));
}

/// Shared handles for leaving playback and repainting the recent grid (Escape path).
struct BackToBrowseCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    win_aspect: Rc<Cell<Option<f64>>>,
    /// Show bars; cancel auto-hide. Call after [gtk::ScrolledWindow::set_visible] for the grid.
    on_browse: Rc<dyn Fn()>,
    undo_shell: gtk::Box,
    undo_label: gtk::Label,
    undo_btn: gtk::Button,
    undo_timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    /// Stack of removed/trashed entries, newest at the end; [Undo] pops from the end.
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
}

/// Show the sheet immediately; save state and repaint cards after a frame while keeping the
/// current file paused as a warm reopen target when the continue list is non-empty.
fn back_to_browse(
    c: &BackToBrowseCtx,
    win: &impl IsA<gtk::Window>,
    gl: &gtk::GLArea,
    recent: &gtk::ScrolledWindow,
    row: &gtk::Box,
    clear_undo: bool,
) {
    cancel_undo_timer(&c.undo_timer);
    if clear_undo {
        *c.undo_remove_stack.borrow_mut() = Vec::new();
        sync_undo_bar(
            &c.undo_label,
            &c.undo_btn,
            &c.undo_shell,
            &c.undo_remove_stack,
        );
    }
    c.win_aspect.set(None);
    *c.last_path.borrow_mut() = None;
    c.sibling_seof.done.set(false);
    c.sibling_seof.stall.set((0.0, 0));
    let paths: Vec<PathBuf> = history::load().into_iter().take(5).collect();
    if paths.is_empty() {
        recent.set_visible(false);
    } else {
        recent.set_visible(true);
    }
    (c.on_browse)();
    win.upcast_ref::<gtk::Window>()
        .set_title(Some(APP_WIN_TITLE));
    gl.queue_render();
    // Cut audio right away; `stop` stays in idlers so a last-frame screenshot can run first.
    if let Some(b) = c.player.borrow().as_ref() {
        let _ = b.mpv.set_property("pause", true);
    }

    if paths.is_empty() {
        let p2 = c.player.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            if let Some(b) = p2.borrow().as_ref() {
                b.snapshot_outgoing_before_leave();
                b.save_playback_state();
                b.stop_playback();
            }
            glib::ControlFlow::Break
        });
        return;
    }

    // FnOnce chain: `idle_add_local_full` requires FnMut, so the grid refill is scheduled from
    // a one-shot idle (paint can run first at DEFAULT_IDLE priority).
    let p_write = c.player.clone();
    let row2 = row.clone();
    let op2 = c.on_open.clone();
    let osl2 = c.on_remove.clone();
    let otr2 = c.on_trash.clone();
    let paths2 = paths;
    let rbb = c.recent_backfill.clone();
    let _ = glib::source::idle_add_local_once(move || {
        if let Some(b) = p_write.borrow().as_ref() {
            b.snapshot_outgoing_before_leave();
        }
        let rbb2 = rbb.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            let v: Vec<CardData> = card_data_list(&paths2);
            recent_view::fill_row(&row2, v, op2.clone(), osl2.clone(), otr2.clone());
            let n = recent_view::ensure_recent_backfill(
                &rbb2,
                &row2,
                op2.clone(),
                osl2.clone(),
                otr2.clone(),
            );
            recent_view::schedule_thumb_backfill(n, paths2.clone());
            glib::ControlFlow::Break
        });
    });
}

/// Enables [gio::SimpleAction] `app.close-video` when the player is ready and the continue grid is hidden.
fn sync_close_video_action(
    a: &gio::SimpleAction,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &impl IsA<gtk::Widget>,
) {
    a.set_enabled(player.borrow().is_some() && !recent.is_visible());
}

/// Enables [gio::SimpleAction] `app.move-to-trash` for a local file in playback (not streams / empty path).
fn sync_trash_action(
    a: &gio::SimpleAction,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &impl IsA<gtk::Widget>,
) {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        a.set_enabled(false);
        return;
    };
    let ok = !recent.is_visible() && local_file_from_mpv(&b.mpv).is_some_and(|p| p.is_file());
    a.set_enabled(ok);
}

/// Hides the window, then (after GTK can draw the hide) saves watch_later/DB, stops, and quits.
fn schedule_quit_persist(
    app: &adw::Application,
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub: &Rc<RefCell<db::SubPrefs>>,
    idle_inhib: &Rc<RefCell<Option<u32>>>,
) {
    win.set_visible(false);
    let p = player.clone();
    let a = app.clone();
    let sp = Rc::clone(sub);
    let ic = Rc::clone(idle_inhib);
    let _ = glib::idle_add_local(move || {
        idle_inhibit::clear(&a, &ic);
        if let Some(b) = p.borrow().as_ref() {
            save_mpv_state(&b.mpv, &sp);
            b.commit_quit();
        }
        a.quit();
        glib::ControlFlow::Break
    });
}
