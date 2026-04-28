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
    /// Before `loadfile`, set mpv speed to **1.0** if it was changed (sibling EOF advance).
    reset_speed_to_normal: bool,
}

/// Load a file, hide the recent grid overlay, show video; [LoadOpts::record] appends to recent history.
/// [play_on_start]: clear `pause` so playback runs after the SQLite resume `start=` is applied.
/// **false** for CLI open-on-launch to respect saved state.
fn try_load(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
) -> Result<(), String> {
    eprintln!(
        "[rhino] try_load: path={} exists={} record={} player_ready={} play={}",
        path.display(),
        path.exists(),
        o.record,
        player.borrow().is_some(),
        o.play_on_start
    );
    let warm_hit = load_file_into_player(path, player, recent_layer, o)?;
    if !warm_hit {
        schedule_reapply_60(player, gl, o);
    }
    *o.last_path.borrow_mut() = std::fs::canonicalize(path).ok();
    if o.record {
        history::record(path);
    }
    win.set_title(Some(title_for_open_path(path).as_str()));
    reveal_ui_after_load(player, win, gl, recent_layer, o, warm_hit);
    if !warm_hit {
        transport_drain_after_loadfile();
        let _ = glib::idle_add_local_once(transport_drain_after_loadfile);
    }
    if let Some(f) = o.on_loaded.clone() {
        glib::source::idle_add_local_once(move || f());
    }
    Ok(())
}

/// Calls `loadfile` on the player, or detects a warm preload hit.
/// Returns `true` if the file was already loaded (warm hit).
fn load_file_into_player(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
) -> Result<bool, String> {
    let mut g = player.borrow_mut();
    let b = g.as_mut().ok_or("Player not ready. Wait for GL init.")?;
    let prev = local_file_from_mpv(&b.mpv).or_else(|| o.last_path.borrow().clone());
    if recent_layer.is_visible() && prev.as_ref().is_some_and(|p| same_open_target(p, path)) {
        eprintln!("[rhino] try_load: warm preload hit");
        return Ok(true);
    }
    // Normalize speed before `loadfile` for sibling auto-advance (mpv keeps `speed`
    // across loadfile within a session; resume position is read from SQLite, not mpv).
    if o.reset_speed_to_normal {
        crate::playback_speed::force_normal(&b.mpv);
    }
    let clear_resume = is_done_enough_to_drop_continue(&b.mpv) && local_file_from_mpv(&b.mpv).is_some();
    let drop_prev = prev.as_ref().is_some_and(|p| {
        !same_open_target(p, path) && is_done_enough_to_drop_continue(&b.mpv)
    });
    if let Err(e) = b.load_file_path(path, clear_resume) {
        eprintln!("[rhino] try_load: loadfile failed: {e}");
        return Err(e);
    }
    eprintln!("[rhino] try_load: loadfile ok");
    if drop_prev {
        if let Some(p) = prev {
            remove_continue_entry(&p);
        }
    }
    Ok(false)
}

/// Schedules two idle passes that apply / re-check the 60fps VapourSynth filter after loadfile.
fn schedule_reapply_60(player: &Rc<RefCell<Option<MpvBundle>>>, gl: &gtk::GLArea, o: &LoadOpts) {
    let Some(r) = o.reapply_60.as_ref() else { return };
    let p = Rc::clone(player);
    let r0 = r.clone();
    let gl0 = gl.clone();
    let _ = glib::idle_add_local_once(move || {
        if let Some(b) = p.borrow().as_ref() {
            let a = video_pref::apply_mpv_video(&b.mpv, &mut r0.vp.borrow_mut(), None);
            if a.smooth_auto_off {
                sync_smooth_60_to_off(&r0.app);
                show_smooth_setup_dialog(&r0.app);
            }
        }
        gl0.queue_render();
        let p2 = Rc::clone(&p);
        let r1 = r0.clone();
        let gl1 = gl0.clone();
        let _ = glib::idle_add_local_once(move || {
            if let Some(b) = p2.borrow().as_ref() {
                let off = video_pref::reapply_60_if_still_missing(&b.mpv, &mut r1.vp.borrow_mut());
                if off {
                    sync_smooth_60_to_off(&r1.app);
                    show_smooth_setup_dialog(&r1.app);
                }
            }
            gl1.queue_render();
        });
    });
}

/// Hides recent grid and kicks off playback (immediate or delayed warm reveal).
fn reveal_ui_after_load(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
    warm_hit: bool,
) {
    let delayed_warm = warm_hit && o.play_on_start;
    if !delayed_warm {
        recent_layer.set_visible(false);
        if let Some(f) = o.on_start.as_ref() {
            f();
        }
    }
    gl.queue_render();
    if o.play_on_start {
        start_playback(player, win, gl, recent_layer, o, delayed_warm);
    }
    if let Some(b) = player.borrow().as_ref() {
        sync_window_aspect_from_mpv(&b.mpv, o.win_aspect.as_ref());
    }
    schedule_window_fit_h_video(Rc::clone(player), win.clone());
}

/// Unpauses mpv; for warm-hit paths, delays reveal by [WARM_REVEAL_DELAY_MS].
fn start_playback(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
    delayed_warm: bool,
) {
    if delayed_warm {
        if let Some(b) = player.borrow().as_ref() {
            resync_warm_continue(&b.mpv);
        }
        let recent = recent_layer.as_ref().clone();
        let win2 = win.clone();
        let gl2 = gl.clone();
        let player2 = player.clone();
        let on_start = o.on_start.clone();
        let _ = glib::timeout_add_local(Duration::from_millis(WARM_REVEAL_DELAY_MS), move || {
            recent.set_visible(false);
            if let Some(f) = on_start.as_ref() { f(); }
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
        let _ = glib::source::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if let Some(b) = p2.borrow().as_ref() {
                let _ = b.mpv.set_property("pause", false);
            }
            glib::ControlFlow::Break
        });
    }
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

/// State for `maybe_advance_sibling_on_eof`: one-shot guard per logical end.
struct SiblingEofState {
    done: Cell<bool>,
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
