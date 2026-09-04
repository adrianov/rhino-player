pub(crate) fn start_preview_frame_pump(
    st: &SeekPreviewState,
    run_id: u64,
    load: &str,
    content_dur: f64,
    seek_sec: f64,
    optical: bool,
) {
    crate::glib_source_drop::drop_glib_source(&st.pump);
    let job = pump_job(run_id, optical, seek_sec, content_dur, load);
    crate::preview_debug::info(format!(
        "pump start run={run_id} seek={seek_sec:.2} dur={content_dur:.2} optical={optical} max_ticks={}",
        job.max_ticks
    ));
    arm_pump_timer(&st.preview, &st.pump, &st.serial, job, &st.gl);
}

fn arm_pump_timer(
    preview: &Rc<RefCell<Option<MpvPreviewGl>>>,
    pump: &Rc<RefCell<Option<glib::SourceId>>>,
    serial: &Rc<Cell<u64>>,
    job: Rc<PumpJob>,
    gl: &gtk::GLArea,
) {
    let gl2 = gl.clone();
    let pr2 = Rc::clone(preview);
    let pump2 = Rc::clone(pump);
    let serial2 = Rc::clone(serial);
    let n = Rc::new(Cell::new(0i32));
    *pump.borrow_mut() = Some(glib::source::timeout_add_local_full(
        VO_PUMP_STEP,
        glib::Priority::DEFAULT,
        move || pump_tick(&pr2, &pump2, &serial2, &n, &job, &gl2),
    ));
}

/// Per-run parameters and tick budget for the VO pump (90 ticks, 180 for optical discs).
fn pump_job(
    run_id: u64,
    optical: bool,
    seek_sec: f64,
    content_dur: f64,
    load: &str,
) -> Rc<PumpJob> {
    Rc::new(PumpJob {
        run_id,
        max_ticks: if optical { 180 } else { 90 },
        optical,
        seek_sec,
        content_dur,
        load_s: load.to_string(),
    })
}

include!("preview_frame_pump/pump_ticks.rs");
