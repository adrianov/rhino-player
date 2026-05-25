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
    do_preview_seek(st, &load_s, content_dur, t, run_id);
    glib::ControlFlow::Break
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
    crate::preview_debug::info(format!(
        "debounce arm run={run_id} hover={:.2}",
        st.hover_t.get()
    ));
    let st2 = Rc::clone(&st);
    let id = glib::source::timeout_add_local_full(
        PREVIEW_DEBOUNCE,
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

fn preview_owner_db(player: &Rc<RefCell<Option<MpvBundle>>>) -> Option<PathBuf> {
    let g = player.borrow();
    let b = g.as_ref()?;
    let shell = b.me_budget_shell_path.borrow().clone();
    crate::playback_entity::open_playback(&b.mpv, shell.as_deref())
        .map(|(ent, _)| ent.db_path())
}

fn do_preview_seek(
    st: &Rc<SeekPreviewState>,
    load_s: &str,
    content_dur: f64,
    t: f64,
    run_id: u64,
) {
    let owner_db = preview_owner_db(&st.player);
    let mut g = st.preview.borrow_mut();
    let Some(pr) = g.as_mut() else {
        crate::preview_debug::warn("do_seek: preview GL/mpv not realised yet");
        return;
    };
    if load_s.is_empty() {
        crate::preview_debug::warn("do_seek: empty load target");
        return;
    }
    let cache = preview_cache_path(load_s);
    let entity_changed = owner_db.as_ref() != st.preview_owner_db.borrow().as_ref();
    let need_load =
        entity_changed || st.loaded_target.borrow().as_deref() != Some(load_s);
    let optical = preview_media_is_optical(load_s);
    crate::preview_debug::info(format!(
        "do_seek load={load_s} t={t:.2} dur={content_dur:.2} need_load={need_load} entity_chg={entity_changed} optical={optical}"
    ));

    if need_load {
        pr.clear_framebuffer(&st.gl);
        prepare_preview_player(&pr.mpv, load_s);
        if let Err(e) = pr.mpv.command("loadfile", &[load_s, "replace"]) {
            crate::preview_debug::warn(format!("loadfile failed: {e:?} ({load_s})"));
            return;
        }
        crate::preview_debug::info(format!(
            "loadfile ok ({})",
            crate::preview_debug::mpv_line(&pr.mpv)
        ));
        *st.loaded_path.borrow_mut() = Some(cache);
        *st.loaded_target.borrow_mut() = Some(load_s.to_string());
        *st.preview_owner_db.borrow_mut() = owner_db;
        drop(g);
        start_preview_frame_pump(
            &st.gl,
            &st.preview,
            &st.pump,
            &st.serial,
            run_id,
            load_s,
            content_dur,
            t,
            optical,
        );
    } else {
        set_preview_tracks(&pr.mpv);
        drop(g);
        start_preview_frame_pump(
            &st.gl,
            &st.preview,
            &st.pump,
            &st.serial,
            run_id,
            load_s,
            content_dur,
            t,
            optical,
        );
    }
}
