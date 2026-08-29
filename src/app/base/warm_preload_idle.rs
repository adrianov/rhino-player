include!("warm_preload_gate.rs");

thread_local! {
    static WARM_CTX: RefCell<Option<Rc<WarmPreloadCtx>>> = const { RefCell::new(None) };
}

/// Whether hover preload started an async `loadfile` or finished synchronously.
enum PreloadOutcome {
    Deferred,
    Ready,
    Failed,
}

pub(crate) fn register_warm_preload_ctx(ctx: Rc<WarmPreloadCtx>) {
    WARM_CTX.with(|s| *s.borrow_mut() = Some(ctx));
}

pub(crate) fn warm_preload_gate_busy() -> bool {
    WARM_CTX.with(|s| s.borrow().as_ref().is_some_and(|c| c.gate.busy()))
}

/// Continue grid visible or a warm `loadfile` in flight — no EOF sibling advance or unpause.
#[must_use]
pub(crate) fn browse_overlay_active(recent: &impl gtk::prelude::WidgetExt) -> bool {
    recent.is_visible()
}

/// Stop warm preload bookkeeping when the user opens a continue card for playback.
pub(crate) fn cancel_warm_preload_for_playback() {
    disarm_warm_path_settle();
    WARM_CTX.with(|s| {
        let guard = s.borrow();
        let Some(c) = guard.as_ref() else {
            return;
        };
        c.gate.cancel();
    });
}

/// Keep mpv paused and resync video chrome while the continue grid stays on screen.
pub(crate) fn warm_preload_hold_browse_pause(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl: &gtk::GLArea,
) {
    let hold = WARM_CTX.with(|s| s.borrow().as_ref().is_some_and(|c| c.recent.is_visible()));
    if !hold {
        return;
    }
    if let Ok(g) = player.try_borrow() {
        if let Some(b) = g.as_ref() {
            let _ = b.mpv.set_property("pause", true);
            b.nudge_browse_video_layout(gl);
        }
    }
}

/// Player RefCell contended — retry the finish on a later idle tick.
fn defer_warm_preload_finish(player: &Rc<RefCell<Option<MpvBundle>>>, want_gen: u32) {
    let p = Rc::clone(player);
    let _ = glib::idle_add_local_once(move || warm_preload_finish_load(&p, want_gen));
}

/// While the continue grid is on screen, keep mpv paused (deferred one idle tick).
fn hold_browse_pause_if_recent_visible(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl: Option<&gtk::GLArea>,
) {
    if gl.as_ref().is_some_and(|_| {
        WARM_CTX.with(|s| s.borrow().as_ref().is_some_and(|c| c.recent.is_visible()))
    }) {
        if let Some(gl) = gl {
            let gl = gl.clone();
            let player2 = Rc::clone(player);
            let _ =
                glib::idle_add_local_once(move || warm_preload_hold_browse_pause(&player2, &gl));
        }
    }
}

/// Same-generation load finished: restore tracks, nudge transport, drain, hold browse pause.
fn finish_warm_preload_sync(player: &Rc<RefCell<Option<MpvBundle>>>) {
    let gl = WARM_CTX.with(|s| s.borrow().as_ref().map(|c| c.gl.clone()));
    warm_preload_apply_resume_audio(player, gl.as_ref());
    transport_nudge_tick();
    let _ = glib::idle_add_local_once(transport_drain_after_loadfile);
    hold_browse_pause_if_recent_visible(player, gl.as_ref());
}

pub(crate) fn warm_preload_finish_load(player: &Rc<RefCell<Option<MpvBundle>>>, want_gen: u32) {
    let cur = match player.try_borrow() {
        Ok(g) => g
            .as_ref()
            .map(crate::mpv_embed::MpvBundle::warm_file_gen)
            .unwrap_or(0),
        Err(_) => {
            defer_warm_preload_finish(player, want_gen);
            return;
        }
    };
    if cur != want_gen {
        warm_preload_notify_loaded();
        return;
    }
    finish_warm_preload_sync(player);
    warm_preload_notify_loaded();
}

fn warm_preload_apply_resume_audio(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl: Option<&gtk::GLArea>,
) {
    if let Ok(g) = player.try_borrow() {
        if let Some(b) = g.as_ref() {
            let shell = b.me_budget_shell_path.borrow();
            let shell_ref = shell.as_deref();
            crate::audio_tracks::restore_saved_audio(&b.mpv, shell_ref);
            crate::audio_tracks::ensure_playable_audio(&b.mpv, shell_ref);
            let pr = crate::db::load_sub();
            let _ = crate::sub_tracks::restore_saved_sub(&b.mpv, &pr, shell_ref);
            // Resume seek after track restore so the seek re-aligns A/V for the reopened decoder.
            b.apply_pending_resume();
        }
    }
    if let Some(gl) = gl {
        warm_preload_hold_browse_pause(player, gl);
    }
}

fn finish_warm_preload_ready_now(player: &Rc<RefCell<Option<MpvBundle>>>, gl: &gtk::GLArea) {
    if player.try_borrow().is_err() {
        let p = Rc::clone(player);
        let gl2 = gl.clone();
        let _ = glib::idle_add_local_once(move || finish_warm_preload_ready_now(&p, &gl2));
        return;
    }
    warm_preload_apply_resume_audio(player, Some(gl));
    let _ = glib::idle_add_local_once(transport_drain_after_loadfile);
    transport_nudge_tick();
}
