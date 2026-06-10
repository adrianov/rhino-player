fn preview_open_ready(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> bool {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    let shell = b.me_budget_shell_path.borrow();
    if crate::playback_entity::open_playback(&b.mpv, shell.as_deref()).is_some() {
        return true;
    }
    let raw = last_path.borrow().clone();
    raw.is_some_and(|p| crate::video_ext::is_openable_media_path(&p))
}

fn preview_open_path(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> Option<PathBuf> {
    preview_open_ready(player, last_path).then_some(PathBuf::new())
}

/// Run debounced preview seek for [SeekPreviewState::hover_t] (does not bump [SeekPreviewState::serial]).
fn execute_preview_seek(st: &Rc<SeekPreviewState>, run_id: u64) -> glib::ControlFlow {
    if st.serial.get() != run_id || !st.enabled.get() {
        if st.serial.get() != run_id {
            crate::preview_debug::warn(format!(
                "debounce aborted run={run_id} serial={} (stale)",
                st.serial.get()
            ));
        } else {
            crate::preview_debug::warn("debounce aborted: preview disabled in prefs");
        }
        return glib::ControlFlow::Break;
    }
    crate::preview_debug::info(format!(
        "debounce fire run={run_id} hover={:.2}",
        st.hover_t.get()
    ));
    let seek = {
        let g = st.player.borrow();
        let Some(b) = g.as_ref() else {
            crate::preview_debug::warn("debounced seek: no main player");
            st.hide();
            return glib::ControlFlow::Break;
        };
        let shell = b.me_budget_shell_path.borrow().clone();
        let bar_d = st.seek_adj.upper();
        let hover = st.hover_t.get();
        let preview_guard = st.preview.borrow();
        let preview_ready = preview_guard.is_some();
        let preview_mpv = preview_guard.as_ref().map(|p| &p.mpv);
        let plan = crate::playback_entity::preview_seek_plan_for_open(
            &b.mpv,
            shell.as_deref(),
            hover,
            bar_d,
            Some(&st.dvd_bar),
            preview_mpv,
        );
        if plan.is_none() {
            let bar_cached = st.dvd_bar.borrow().is_some();
            crate::preview_debug::warn(format!(
                "no seek plan hover={hover:.2} bar={bar_d:.2} shell={} bar_cache={bar_cached} preview_gl={preview_ready}",
                shell
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "none".into())
            ));
        } else {
            crate::preview_debug::info(format!(
                "plan hover={hover:.2} bar={bar_d:.2} load={}",
                plan.as_ref().map(|p| p.load.as_str()).unwrap_or("")
            ));
        }
        plan.map(|plan| {
            let t = cap_preview_seek_time(plan.local_sec, plan.content_dur);
            (plan.load, plan.content_dur, t)
        })
    };
    let Some((load_s, content_dur, t)) = seek else {
        st.hide();
        return glib::ControlFlow::Break;
    };
    do_preview_seek(st, &load_s, content_dur, t, run_id, false);
    glib::ControlFlow::Break
}

fn preview_debounce(st: &SeekPreviewState) -> Duration {
    let smooth = st
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| crate::video_pref::vf_chain_has_vapoursynth(&b.mpv));
    if smooth {
        Duration::from_millis(200)
    } else {
        PREVIEW_DEBOUNCE
    }
}

/// Arm one trailing debounce; motion updates `hover_t` without resetting the timer.
pub(crate) fn arm_preview_debounce(st: Rc<SeekPreviewState>) {
    if st.deb.borrow().is_some() {
        crate::preview_debug::log(format!(
            "debounce coalesce hover={:.2}",
            st.hover_t.get()
        ));
        return;
    }
    let run_id = st.serial.get();
    let deb = preview_debounce(&st);
    crate::preview_debug::info(format!(
        "debounce arm run={run_id} hover={:.2} ms={}",
        st.hover_t.get(),
        deb.as_millis()
    ));
    let st2 = Rc::clone(&st);
    let id = glib::source::timeout_add_local_full(
        deb,
        glib::Priority::DEFAULT,
        move || {
            let _ = st2.deb.borrow_mut().take();
            execute_preview_seek(&st2, run_id)
        },
    );
    *st.deb.borrow_mut() = Some(id);
}

pub(crate) fn schedule_preview_seek(st: Rc<SeekPreviewState>) {
    arm_preview_debounce(st);
}

/// Seek immediately (no debounce) — reopen after hide or GL realise while hover is active.
pub(crate) fn run_preview_seek_now(st: &Rc<SeekPreviewState>) {
    crate::glib_source_drop::drop_glib_source(st.deb.as_ref());
    let run_id = st.serial.get();
    execute_preview_seek_instant(st, run_id);
}

fn execute_preview_seek_instant(st: &Rc<SeekPreviewState>, run_id: u64) {
    if st.serial.get() != run_id || !st.enabled.get() {
        return;
    }
    let seek = {
        let g = st.player.borrow();
        let Some(b) = g.as_ref() else {
            st.hide();
            return;
        };
        let shell = b.me_budget_shell_path.borrow().clone();
        let bar_d = st.seek_adj.upper();
        let hover = st.hover_t.get();
        let preview_guard = st.preview.borrow();
        let preview_mpv = preview_guard.as_ref().map(|p| &p.mpv);
        crate::playback_entity::preview_seek_plan_for_open(
            &b.mpv,
            shell.as_deref(),
            hover,
            bar_d,
            Some(&st.dvd_bar),
            preview_mpv,
        )
        .map(|plan| {
            let t = cap_preview_seek_time(plan.local_sec, plan.content_dur);
            (plan.load, plan.content_dur, t)
        })
    };
    let Some((load_s, content_dur, t)) = seek else {
        st.hide();
        return;
    };
    do_preview_seek(st, &load_s, content_dur, t, run_id, true);
}

include!("preview_do_seek.rs");
