fn preview_owner_db(player: &Rc<RefCell<Option<MpvBundle>>>) -> Option<PathBuf> {
    let g = player.borrow();
    let b = g.as_ref()?;
    crate::playback_entity::open_playback(&b.mpv, budget_shell_path(b).as_deref())
        .map(|(ent, _)| ent.db_path())
}

/// True while a load for exactly this target is already pumping.
fn preview_load_in_flight(st: &SeekPreviewState, load_s: &str) -> bool {
    st.pump.borrow().is_some() && st.loaded_target.borrow().as_deref() == Some(load_s)
}

/// (needs reload, aux VO ready, playback entity changed since the cached clip).
fn preview_load_state(
    st: &SeekPreviewState,
    pr: &MpvPreviewGl,
    owner_db: &Option<PathBuf>,
    load_s: &str,
) -> (bool, bool, bool) {
    let entity_changed = owner_db.as_ref() != st.preview_owner_db.borrow().as_ref();
    let vo_ready = pr.mpv.get_property::<bool>("vo-configured") == Ok(true);
    let need_load =
        entity_changed || st.loaded_target.borrow().as_deref() != Some(load_s) || !vo_ready;
    (need_load, vo_ready, entity_changed)
}

fn log_do_seek(
    load_s: &str,
    t: f64,
    content_dur: f64,
    need_load: bool,
    entity_changed: bool,
    vo_ready: bool,
    optical: bool,
) {
    crate::preview_debug::info(format!(
        "do_seek load={load_s} t={t:.2} dur={content_dur:.2} need_load={need_load} entity_chg={entity_changed} vo_ready={vo_ready} optical={optical}"
    ));
}

fn do_preview_seek(
    st: &Rc<SeekPreviewState>,
    load_s: &str,
    content_dur: f64,
    t: f64,
    run_id: u64,
    instant: bool,
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
    let optical = preview_media_is_optical(load_s);
    let (need_load, vo_ready, entity_changed) = preview_load_state(st, pr, &owner_db, load_s);
    log_do_seek(
        load_s,
        t,
        content_dur,
        need_load,
        entity_changed,
        vo_ready,
        optical,
    );
    drop(g);
    if need_load {
        reload_finish(st, owner_db, load_s, content_dur, t, optical, run_id);
    } else {
        warm_finish(
            st,
            SeekTarget {
                load_s,
                content_dur,
                t,
                optical,
            },
            instant,
            vo_ready,
            run_id,
        );
    }
}

include!("preview_do_seek/warm_reload.rs");
