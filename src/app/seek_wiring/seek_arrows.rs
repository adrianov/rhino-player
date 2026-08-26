fn quick_seek(ctx: &SeekCtx, v: f64) {
    let s = format!("{v:.4}");
    main_player_seek_keyframes(
        &SeekKeyframeParams {
            player: &ctx.player,
            gl: &ctx.gl,
            smooth_seek_debounce: &ctx.smooth_seek_debounce,
            resume_after_seek_idle: &ctx.resume_after_seek_idle,
            play_toggle: &ctx.play_toggle,
            dvd_bar: Some(&ctx.dvd_bar),
        },
        SeekKeyframeKind::ScaleOrExternal,
        &s,
    );
}

struct SeekArrowDeps<'a> {
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    seek: &'a gtk::Scale,
    seek_sync: &'a Rc<Cell<bool>>,
    time_left: &'a gtk::Label,
    gl: &'a gtk::GLArea,
    smooth_seek_debounce: &'a Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: &'a Rc<Cell<bool>>,
    play_toggle: &'a PlayToggleCtx,
    dvd_bar: Option<&'a Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>>,
}

#[must_use]
fn dvd_title_pos(
    b: &MpvBundle,
    ch: &std::path::Path,
    local: f64,
    live: f64,
    dvd_bar: Option<&Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>>,
) -> Option<(f64, f64)> {
    if let Some(slot) = dvd_bar {
        let guard = slot.borrow();
        if let Some(ref bar) = *guard {
            return Some((bar.transport_global_pos(b, ch, local), bar.total_sec()));
        }
    }
    crate::dvd_vob_timeline::DvdBarState::build(ch, live)
        .map(|bar| (bar.transport_global_pos(b, ch, local), bar.total_sec()))
}

/// Clamps a possibly non-finite mpv property to a non-negative finite time.
fn finite_nonneg(v: f64) -> f64 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

fn arrow_seek_pos_len(
    b: &MpvBundle,
    seek: &gtk::Scale,
    dvd_bar: Option<&Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>>,
) -> Option<(f64, f64)> {
    let pos = b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    let dur = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let (pos, dur) = dvd_adjusted_pos_dur(b, pos, dur, dvd_bar);
    let adj_u = finite_nonneg(seek.adjustment().upper());
    let len = if adj_u > 0.0 {
        adj_u
    } else {
        finite_nonneg(dur)
    };
    (len > 0.0).then_some((finite_nonneg(pos), len))
}

/// Replaces raw mpv position / duration with DVD global position and title length when known.
fn dvd_adjusted_pos_dur(
    b: &MpvBundle,
    pos: f64,
    dur: f64,
    dvd_bar: Option<&Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>>,
) -> (f64, f64) {
    let shell = b.me_budget_shell_path.borrow().clone();
    if let Some(ch) = crate::media_probe::local_file_from_mpv(&b.mpv).or(shell) {
        if let Some((g, total)) = dvd_title_pos(b, &ch, pos, finite_nonneg(dur), dvd_bar) {
            return (g, total);
        }
    }
    (pos, dur)
}

/// Steps **playback position** by `delta_sec` (e.g. −5 / +5 for arrow keys); keeps UI scale + clock aligned.
fn seek_arrow_step(d: &SeekArrowDeps<'_>, delta_sec: f64) {
    let nt = {
        let g = d.player.borrow();
        let Some(b) = g.as_ref() else {
            return;
        };
        let Some((pos, len)) = arrow_seek_pos_len(b, d.seek, d.dvd_bar) else {
            return;
        };
        (pos + delta_sec).clamp(0.0, len)
    };
    let s_abs = format!("{nt:.4}");
    crate::user_action_log::act(format!("seek arrow {delta_sec:+.0}s -> t={s_abs}s"));
    main_player_seek_keyframes(
        &SeekKeyframeParams {
            player: d.player,
            gl: d.gl,
            smooth_seek_debounce: d.smooth_seek_debounce,
            resume_after_seek_idle: d.resume_after_seek_idle,
            play_toggle: d.play_toggle,
            dvd_bar: d.dvd_bar,
        },
        SeekKeyframeKind::ArrowBurst,
        &s_abs,
    );
    sync_arrow_seek_ui(d, nt);
}

fn sync_arrow_seek_ui(d: &SeekArrowDeps<'_>, nt: f64) {
    d.seek_sync.set(true);
    d.seek.set_value(nt);
    d.seek_sync.set(false);
    sync_time_left_label(d.time_left, nt);
}
