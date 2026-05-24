fn preview_open_path(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> Option<PathBuf> {
    let g = player.borrow();
    let b = g.as_ref()?;
    let shell = b.me_budget_shell_path.borrow();
    if let Some(s) = preview_load_path(&b.mpv, shell.as_deref()) {
        return Some(preview_cache_path(&s));
    }
    let raw = last_path.borrow().clone()?;
    if !crate::video_ext::is_openable_media_path(&raw) {
        return None;
    }
    let resolved = crate::video_ext::resolve_open_media_path(&raw);
    resolved.to_str().map(preview_cache_path)
}

pub(crate) fn schedule_preview_seek(st: Rc<SeekPreviewState>) {
    let run_id = st.serial.get().wrapping_add(1);
    st.serial.set(run_id);
    let st2 = Rc::clone(&st);
    let id = glib::source::timeout_add_local_full(
        PREVIEW_DEBOUNCE,
        glib::Priority::LOW,
        move || {
            let _ = st2.deb.borrow_mut().take();
            if st2.serial.get() != run_id || !st2.enabled.get() {
                return glib::ControlFlow::Break;
            }
            let seek = {
                let g = st2.player.borrow();
                let Some(b) = g.as_ref() else {
                    st2.hide();
                    return glib::ControlFlow::Break;
                };
                let shell = b.me_budget_shell_path.borrow().clone();
                let bar_d = st2.seek_adj.upper();
                let hover = st2.hover_t.get();
                if let Some(pt) = crate::dvd_vob_timeline::preview_target(
                    &b.mpv,
                    shell.as_deref(),
                    hover,
                    Some(&st2.dvd_bar),
                ) {
                    let cap = if pt.chapter_dur > 0.0 {
                        pt.chapter_dur
                    } else {
                        bar_d
                    };
                    let t = cap_preview_seek_time(pt.local_sec, cap);
                    Some((pt.load, cap, t))
                } else if let Some(load_s) = preview_load_path(&b.mpv, shell.as_deref()) {
                    let content_dur = preview_hover_duration(
                        bar_d,
                        &b.mpv,
                        st2.preview.borrow().as_ref().map(|p| &p.mpv),
                    );
                    let t = cap_preview_seek_time(hover, content_dur);
                    Some((load_s, content_dur, t))
                } else {
                    None
                }
            };
            let Some((load_s, content_dur, t)) = seek else {
                st2.hide();
                return glib::ControlFlow::Break;
            };
            do_preview_seek(&st2, &load_s, content_dur, t, run_id);
            glib::ControlFlow::Break
        },
    );
    *st.deb.borrow_mut() = Some(id);
}

fn do_preview_seek(
    st: &Rc<SeekPreviewState>,
    load_s: &str,
    content_dur: f64,
    t: f64,
    run_id: u64,
) {
    let mut g = st.preview.borrow_mut();
    let Some(pr) = g.as_mut() else {
        return;
    };
    if load_s.is_empty() {
        return;
    }
    let cache = preview_cache_path(load_s);
    let need_load = st.loaded_target.borrow().as_deref() != Some(load_s);
    let optical = preview_media_is_optical(load_s);

    if need_load {
        prepare_preview_player(&pr.mpv, load_s);
        if let Err(e) = pr.mpv.command("loadfile", &[load_s, "replace"]) {
            eprintln!("[rhino] seek preview: loadfile failed: {e:?} ({load_s})");
            return;
        }
        *st.loaded_path.borrow_mut() = Some(cache);
        *st.loaded_target.borrow_mut() = Some(load_s.to_string());
        drop(g);
        start_preview_frame_pump(
            &st.gl,
            &st.preview,
            &st.pump,
            &st.serial,
            run_id,
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
            content_dur,
            t,
            optical,
        );
    }
}
