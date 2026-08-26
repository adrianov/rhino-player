/// Everything one pump run needs across its ticks.
struct PumpJob {
    run_id: u64,
    max_ticks: i32,
    optical: bool,
    seek_sec: f64,
    content_dur: f64,
    load_s: String,
}

/// One VO-pump tick: gate, drain events, wait for readiness, then seek + render.
fn pump_tick(
    pr_slot: &Rc<RefCell<Option<MpvPreviewGl>>>,
    pump: &Rc<RefCell<Option<glib::SourceId>>>,
    serial: &Rc<Cell<u64>>,
    n: &Rc<Cell<i32>>,
    job: &PumpJob,
    gl: &gtk::GLArea,
) -> glib::ControlFlow {
    if let Some(flow) = pump_tick_gate(pr_slot, pump, serial, n, job) {
        return flow;
    }
    let mut p = pr_slot.borrow_mut();
    let Some(pr) = p.as_mut() else {
        *pump.borrow_mut() = None;
        crate::preview_debug::warn(format!(
            "pump run={} tick={}: no preview player",
            job.run_id,
            n.get()
        ));
        return glib::ControlFlow::Break;
    };
    while pr.mpv.wait_event(0.0).is_some() {}
    if pump_wait_pending(pr, n, job) {
        return glib::ControlFlow::Continue;
    }
    pump_seek_tick(pr, gl, job, n);
    *pump.borrow_mut() = None;
    glib::ControlFlow::Break
}

/// Staleness and per-run tick budget; `Some` ends the pump with the given flow.
fn pump_tick_gate(
    pr_slot: &Rc<RefCell<Option<MpvPreviewGl>>>,
    pump: &Rc<RefCell<Option<glib::SourceId>>>,
    serial: &Rc<Cell<u64>>,
    n: &Rc<Cell<i32>>,
    job: &PumpJob,
) -> Option<glib::ControlFlow> {
    if serial.get() != job.run_id {
        *pump.borrow_mut() = None;
        crate::preview_debug::log(format!("pump run={} cancelled (serial stale)", job.run_id));
        return Some(glib::ControlFlow::Break);
    }
    n.set(n.get() + 1);
    if n.get() > job.max_ticks {
        return Some(pump_budget_exhausted(pr_slot, pump, job));
    }
    None
}

/// Tick budget exhausted: tear down and report the aux player's last state.
fn pump_budget_exhausted(
    pr_slot: &Rc<RefCell<Option<MpvPreviewGl>>>,
    pump: &Rc<RefCell<Option<glib::SourceId>>>,
    job: &PumpJob,
) -> glib::ControlFlow {
    let snap = pr_slot
        .borrow()
        .as_ref()
        .map(|pr| crate::preview_debug::mpv_line(&pr.mpv))
        .unwrap_or_else(|| "no preview".into());
    *pump.borrow_mut() = None;
    crate::preview_debug::warn(format!(
        "pump timeout run={run_id} ticks={max_ticks} {snap}",
        run_id = job.run_id,
        max_ticks = job.max_ticks
    ));
    glib::ControlFlow::Break
}

/// True while vo-configured or an optical chain-head probe has not settled yet.
fn pump_wait_pending(pr: &MpvPreviewGl, n: &Rc<Cell<i32>>, job: &PumpJob) -> bool {
    if pump_waits_vo(pr, n, job.run_id) {
        return true;
    }
    job.optical && pump_waits_chain_head(&job.load_s, pr, n, job.run_id)
}

/// True while vo-configured has not settled yet (progress logged every 15 ticks).
fn pump_waits_vo(pr: &MpvPreviewGl, n: &Rc<Cell<i32>>, run_id: u64) -> bool {
    if pr.mpv.get_property::<bool>("vo-configured") == Ok(true) {
        return false;
    }
    if n.get() == 1 || n.get() % 15 == 0 {
        crate::preview_debug::log(format!(
            "pump run={run_id} tick={}: waiting vo-configured ({})",
            n.get(),
            crate::preview_debug::mpv_line(&pr.mpv)
        ));
    }
    true
}

/// True while an optical title-chain head still lacks its probed duration (logged every 15 ticks).
fn pump_waits_chain_head(load_s: &str, pr: &MpvPreviewGl, n: &Rc<Cell<i32>>, run_id: u64) -> bool {
    let chapter = std::path::Path::new(load_s);
    if !crate::dvd_vob_mpv_probe::is_title_chain_head(chapter)
        || crate::dvd_vob_timeline::chain_head_mpv_ready(chapter, &pr.mpv)
    {
        return false;
    }
    if n.get() == 1 || n.get() % 15 == 0 {
        crate::preview_debug::log(format!(
            "pump run={run_id} tick={}: waiting chain-head duration ({})",
            n.get(),
            crate::preview_debug::mpv_line(&pr.mpv)
        ));
    }
    true
}

/// Final phase of a ready tick: seek the aux player and queue a render on success.
fn pump_seek_tick(pr: &MpvPreviewGl, gl: &gtk::GLArea, job: &PumpJob, n: &Rc<Cell<i32>>) {
    let t = cap_preview_seek_time(job.seek_sec, job.content_dur);
    let seek_ok = preview_run_seek(&pr.mpv, &job.load_s, t, job.optical);
    crate::preview_debug::log(format!(
        "pump run={} tick={} seek={t:.2} ok={seek_ok} gl={}x{} ({})",
        job.run_id,
        n.get(),
        gl.width(),
        gl.height(),
        crate::preview_debug::mpv_line(&pr.mpv)
    ));
    if !seek_ok {
        crate::preview_debug::warn(format!(
            "pump run={} seek failed t={:.2} optical={}",
            job.run_id, t, job.optical
        ));
    }
    if seek_ok {
        gl.queue_render();
        crate::preview_debug::info(format!(
            "pump done run={} tick={} seek={t:.2} ({})",
            job.run_id,
            n.get(),
            crate::preview_debug::mpv_line(&pr.mpv)
        ));
    }
}
