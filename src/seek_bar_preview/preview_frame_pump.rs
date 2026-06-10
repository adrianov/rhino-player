pub(crate) fn start_preview_frame_pump(
    gl: &gtk::GLArea,
    preview: &Rc<RefCell<Option<MpvPreviewGl>>>,
    pump: &Rc<RefCell<Option<glib::SourceId>>>,
    serial: &Rc<Cell<u64>>,
    run_id: u64,
    load: &str,
    content_dur: f64,
    seek_sec: f64,
    optical: bool,
) {
    crate::glib_source_drop::drop_glib_source(pump.as_ref());
    let gl2 = gl.clone();
    let pr2 = Rc::clone(preview);
    let pump2 = Rc::clone(pump);
    let serial2 = Rc::clone(serial);
    let n = Rc::new(Cell::new(0i32));
    let max_ticks = if optical { 180 } else { 90 };
    crate::preview_debug::info(format!(
        "pump start run={run_id} seek={seek_sec:.2} dur={content_dur:.2} optical={optical} max_ticks={max_ticks}"
    ));
    let load_s = load.to_string();
    let id = glib::source::timeout_add_local_full(VO_PUMP_STEP, glib::Priority::DEFAULT, move || {
        if serial2.get() != run_id {
            *pump2.borrow_mut() = None;
            crate::preview_debug::log(format!("pump run={run_id} cancelled (serial stale)"));
            return glib::ControlFlow::Break;
        }
        n.set(n.get() + 1);
        if n.get() > max_ticks {
            *pump2.borrow_mut() = None;
            let snap = pr2
                .borrow()
                .as_ref()
                .map(|pr| crate::preview_debug::mpv_line(&pr.mpv))
                .unwrap_or_else(|| "no preview".into());
            crate::preview_debug::warn(format!(
                "pump timeout run={run_id} ticks={max_ticks} {snap}"
            ));
            return glib::ControlFlow::Break;
        }
        let mut p = pr2.borrow_mut();
        let Some(pr) = p.as_mut() else {
            *pump2.borrow_mut() = None;
            crate::preview_debug::warn(format!("pump run={run_id} tick={}: no preview player", n.get()));
            return glib::ControlFlow::Break;
        };
        while pr.mpv.wait_event(0.0).is_some() {}
        if pr.mpv.get_property::<bool>("vo-configured") != Ok(true) {
            if n.get() == 1 || n.get() % 15 == 0 {
                crate::preview_debug::log(format!(
                    "pump run={run_id} tick={}: waiting vo-configured ({})",
                    n.get(),
                    crate::preview_debug::mpv_line(&pr.mpv)
                ));
            }
            return glib::ControlFlow::Continue;
        }
        let chapter = std::path::Path::new(&load_s);
        if optical
            && crate::dvd_vob_mpv_probe::is_title_chain_head(chapter)
            && !crate::dvd_vob_timeline::chain_head_mpv_ready(chapter, &pr.mpv)
        {
            if n.get() == 1 || n.get() % 15 == 0 {
                crate::preview_debug::log(format!(
                    "pump run={run_id} tick={}: waiting chain-head duration ({})",
                    n.get(),
                    crate::preview_debug::mpv_line(&pr.mpv)
                ));
            }
            return glib::ControlFlow::Continue;
        }
        let t = cap_preview_seek_time(seek_sec, content_dur);
        let seek_ok = preview_run_seek(&pr.mpv, &load_s, t, optical);
        crate::preview_debug::log(format!(
            "pump run={run_id} tick={} seek={t:.2} ok={seek_ok} gl={}x{} ({})",
            n.get(),
            gl2.width(),
            gl2.height(),
            crate::preview_debug::mpv_line(&pr.mpv)
        ));
        if !seek_ok {
            crate::preview_debug::warn(format!(
                "pump run={run_id} seek failed t={t:.2} optical={optical}"
            ));
        }
        if seek_ok {
            gl2.queue_render();
            crate::preview_debug::info(format!(
                "pump done run={run_id} tick={} seek={t:.2} ({})",
                n.get(),
                crate::preview_debug::mpv_line(&pr.mpv)
            ));
        }
        *pump2.borrow_mut() = None;
        glib::ControlFlow::Break
    });
    *pump.borrow_mut() = Some(id);
}
