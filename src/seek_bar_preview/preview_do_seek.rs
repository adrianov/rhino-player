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
    let cache = preview_cache_path(load_s);
    let entity_changed = owner_db.as_ref() != st.preview_owner_db.borrow().as_ref();
    let vo_ready = pr.mpv.get_property::<bool>("vo-configured") == Ok(true);
    let need_load = entity_changed
        || st.loaded_target.borrow().as_deref() != Some(load_s)
        || !vo_ready;
    let optical = preview_media_is_optical(load_s);
    crate::preview_debug::info(format!(
        "do_seek load={load_s} t={t:.2} dur={content_dur:.2} need_load={need_load} entity_chg={entity_changed} vo_ready={vo_ready} optical={optical}"
    ));

    if need_load {
        let load_in_flight = st.pump.borrow().is_some()
            && st.loaded_target.borrow().as_deref() == Some(load_s);
        if load_in_flight {
            crate::preview_debug::info(format!(
                "do_seek load in flight, pump only seek={t:.2} ({load_s})"
            ));
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
            return;
        }
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
        let t = cap_preview_seek_time(t, content_dur);
        if instant && vo_ready && preview_run_seek(&pr.mpv, load_s, t, optical) {
            crate::preview_debug::info(format!(
                "do_seek warm instant seek={t:.2} ({})",
                crate::preview_debug::mpv_line(&pr.mpv)
            ));
            st.gl.queue_render();
            return;
        }
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
