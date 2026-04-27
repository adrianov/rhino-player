/// Wires the bottom seek bar and handles VapourSynth paused-seek redraws.
fn wire_seek_control(
    seek: &gtk::Scale,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl: &gtk::GLArea,
    seek_sync: Rc<Cell<bool>>,
) {
    let p_seek = player.clone();
    let gl_seek = gl.clone();
    seek.connect_value_changed(move |r| {
        if seek_sync.get() {
            return;
        }
        let needs_pump = {
            let g = p_seek.borrow();
            let Some(b) = g.as_ref() else {
                return;
            };
            let was_paused = b.mpv.get_property::<bool>("pause").unwrap_or(false);
            let was_vapoursynth =
                was_paused && video_pref::clear_vapoursynth_for_paused_seek(&b.mpv);
            let s = format!("{:.4}", r.value());
            if b.mpv
                .command("seek", &[s.as_str(), "absolute+keyframes"])
                .is_err()
            {
                let _ = b.mpv.set_property("time-pos", r.value());
            }
            was_paused && !was_vapoursynth && video_pref::has_vapoursynth_vf(&b.mpv)
        };
        if needs_pump {
            pump_paused_vapoursynth(&p_seek, &gl_seek);
        } else {
            gl_seek.queue_render();
        }
    });
}

fn pump_paused_vapoursynth(player: &Rc<RefCell<Option<MpvBundle>>>, gl: &gtk::GLArea) {
    if let Some(b) = player.borrow().as_ref() {
        let _ = b.mpv.command("frame-step", &[]);
        let _ = b.mpv.set_property("pause", true);
    }
    gl.queue_render();
    let p = player.clone();
    let g = gl.clone();
    let _ = glib::timeout_add_local(Duration::from_millis(120), move || {
        if let Some(b) = p.borrow().as_ref() {
            let _ = b.mpv.set_property("pause", true);
        }
        g.queue_render();
        glib::ControlFlow::Break
    });
}
