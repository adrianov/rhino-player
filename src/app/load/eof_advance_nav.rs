/// Advance to the next sibling only on mpv **natural** end: `eof-reached` or `EndFile` with EOF reason.
/// `sibling_eof_done` allows one `try_load` per logical end; cleared when `eof-reached` becomes false.
#[allow(clippy::too_many_arguments)]
fn maybe_advance_sibling_on_eof(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent: &gtk::Box,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
    seof: &SiblingEofState,
    exit_after_current: &Rc<Cell<bool>>,
    app: &adw::Application,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    idle_inhib: &Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    teardown_after_draw: &Rc<Cell<bool>>,
    on_start: &Rc<dyn Fn()>,
    win_aspect: Rc<WinAspectCell>,
    on_loaded: Option<Rc<dyn Fn()>>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
    on_open_fail: &Rc<dyn Fn(String)>,
) {
    let g = match player.try_borrow() {
        Ok(b) => b,
        Err(_) => return,
    };
    let Some(pl) = g.as_ref() else {
        return;
    };
    // Continue grid / warm hover: paused preload only — no sibling auto-advance (would call try_load with play).
    if crate::app::browse_overlay_active(recent) {
        return;
    }
    if seof.done.get() {
        return;
    }
    if exit_after_current.get() {
        drop(g);
        quit_after_current_eof(
            app,
            win,
            gl,
            player,
            sub_pref,
            idle_inhib,
            teardown_after_draw,
        );
        return;
    }
    let Some(finished) = eof_finished_target(pl, seof, last_path) else {
        return;
    };
    if seof
        .incomplete_hold
        .hold_instead_of_advance(&pl.mpv, &finished)
    {
        return;
    }
    drop(g);
    advance_to_next_sibling(
        finished,
        LoadTargets {
            player,
            win,
            gl,
            recent,
        },
        SiblingReloadRefs {
            seof,
            last_path,
            video_pref,
            on_start,
            win_aspect: &win_aspect,
            on_open_fail,
        },
        &on_loaded,
        hdr_title_mirror,
        playback_focus,
    );
}

/// Quit-from-EOF: log intent and run the deferred persistence chain.
fn quit_after_current_eof(
    app: &adw::Application,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
    idle_inhib: &Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    teardown_after_draw: &Rc<Cell<bool>>,
) {
    eprintln!("[rhino] quit: exit after current video");
    schedule_quit_persist(
        app,
        win,
        gl,
        player,
        sub_pref,
        idle_inhib,
        teardown_after_draw,
    );
}

include!("eof_advance_reload.rs");

/// Bottom-bar **Previous** / **Next** tooltips: humanized **base name** of the target in folder/sibling
/// order; [can] is from [SiblingEofState::nav_sensitivity].
fn sibling_bar_tooltip(is_prev: bool, can: bool, cur: Option<&Path>) -> String {
    if !can {
        return nav_tip_unavailable(is_prev).to_string();
    }
    let Some(c) = cur else {
        return nav_tip_open(is_prev).to_string();
    };
    let t = if is_prev {
        sibling_advance::prev_before_current(c)
    } else {
        sibling_advance::next_after_eof(c)
    };
    let Some(t) = t else {
        // Rare if [can] and [cur] match [nav_sensitivity]; keep a neutral line if paths diverge.
        return nav_tip_neutral(is_prev).to_string();
    };
    humanized_nav_target(&t)
}

/// Humanized display name for a sibling target; lossy UTF-8 from `OsStr`, DVD disc-root aware,
/// humanized like window title / continue cards.
fn humanized_nav_target(t: &Path) -> String {
    let label_path = crate::video_ext::dvd_disc_root(t).unwrap_or_else(|| t.to_path_buf());
    let raw = label_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| label_path.to_string_lossy().into_owned());
    crate::human_media_title::human_media_title(raw.as_str())
}

/// Tooltip when navigation is disabled for the current folder state.
fn nav_tip_unavailable(is_prev: bool) -> &'static str {
    if is_prev {
        "No previous file in folder order"
    } else {
        "No next file in folder order"
    }
}

/// Tooltip when navigation is enabled but no current file is known.
fn nav_tip_open(is_prev: bool) -> &'static str {
    if is_prev {
        "Open previous in folder order"
    } else {
        "Open next in folder order"
    }
}

/// Neutral fallback when the cached sensitivity and a fresh walk disagree.
fn nav_tip_neutral(is_prev: bool) -> &'static str {
    if is_prev {
        "Previous in folder order"
    } else {
        "Next in folder order"
    }
}
