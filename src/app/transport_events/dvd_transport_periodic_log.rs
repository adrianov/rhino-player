// Periodic DVD unified-timeline transport diagnostics (included from `deferred_resync.rs`).

use std::sync::atomic::AtomicU32;

static DVD_TRANSPORT_LOG_TICK: AtomicU32 = AtomicU32::new(0);

fn maybe_dvd_transport_periodic_log(ctx: &TransportCtx, bar_pos: f64, bar_dur: f64) {
    if !crate::dvd_vob_log::dvd_transport_log_enabled() {
        return;
    }
    let n = DVD_TRANSPORT_LOG_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if n % 5 != 0 {
        return;
    }
    let Ok(g) = ctx.player.try_borrow() else {
        return;
    };
    let Some(b) = g.as_ref() else {
        return;
    };
    let shell = b.me_budget_shell_path.borrow().clone();
    let Some(chapter) = crate::playback_entity::transport_chapter_path(
        ctx.recent_visible.get(),
        ctx.eof.last_path.borrow().clone(),
        Some(&b.mpv),
        shell.as_deref(),
    ) else {
        return;
    };
    let entity = crate::playback_entity::PlaybackEntity::resolve(&chapter);
    if !entity.has_unified_timeline() {
        return;
    }
    let mpv_pos = b.mpv.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
    let mpv_dur = b
        .mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    let playback = b
        .mpv
        .get_property::<f64>("playback-time")
        .ok()
        .filter(|t| t.is_finite() && *t >= 0.0)
        .unwrap_or(f64::NAN);
    let hold = b.dvd_hold_global.get();
    let sync = b.dvd_chain_bar_sync.get();
    let cross = b.chapter_cross_load_busy();
    let scrub = b.chapter_scrub_resume_pending();
    let bar = ctx.dvd_bar.borrow();
    let seg = bar
        .as_ref()
        .and_then(|bar| bar.tl.index_of(&chapter).map(|i| bar.chapter_dur_at(i)))
        .unwrap_or(0.0);
    let implausible = bar.as_ref().is_some_and(|bar| {
        !bar.tl.ifo_segment_local_plausible(&chapter, mpv_pos.max(0.0))
    });
    let chain = crate::dvd_vob_mpv_probe::is_title_chain_head(&chapter);
    crate::dvd_vob_log::dvd_transport_log(format!(
        "tick={n} chapter={} mpv_pos={mpv_pos:.2} mpv_dur={mpv_dur:.1} playback={playback:.2} \
         bar_pos={bar_pos:.2} bar_dur={bar_dur:.1} ifo_seg={seg:.1} hold={hold:?} sync={sync:?} \
         cross={cross} scrub={scrub} chain={chain} implausible={implausible}",
        chapter.file_name().and_then(|s| s.to_str()).unwrap_or("?")
    ));
}
