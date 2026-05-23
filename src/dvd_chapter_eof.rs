// DVD mid-title chapter EOF: detect tail of open `.vob` and load the next chapter.

use libmpv2::Mpv;

/// Open chapter near EOF: tail of mpv `duration` or `eof-reached`.
#[must_use]
pub fn chapter_local_at_eof(mpv: &Mpv) -> bool {
    if mpv.get_property::<bool>("eof-reached").unwrap_or(false) {
        return true;
    }
    let ldur = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    let lpos = mpv
        .get_property::<f64>("time-pos")
        .ok()
        .filter(|p| p.is_finite() && *p >= 0.0)
        .unwrap_or(0.0);
    ldur > 0.0 && (ldur - lpos) <= crate::app::TICK_EOF_TAIL_SEC
}

/// Load the next chapter in the same DVD title when the open file ends but the title has not.
#[must_use]
pub fn advance_title_chapter_eof(
    player: &std::rc::Rc<std::cell::RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    bar: &DvdBarState,
) -> bool {
    let Ok(mut g) = player.try_borrow_mut() else {
        return false;
    };
    let Some(b) = g.as_mut() else {
        return false;
    };
    if !chapter_local_at_eof(&b.mpv) {
        return false;
    }
    let shell = b.me_budget_shell_path.borrow().clone();
    let Some(chapter) = open_dvd_chapter_path(&b.mpv, shell.as_deref()) else {
        return false;
    };
    let lpos = b
        .mpv
        .get_property::<f64>("time-pos")
        .ok()
        .filter(|p| p.is_finite() && *p >= 0.0)
        .unwrap_or(0.0);
    let global = bar.global_pos(&chapter, lpos);
    if (bar.total_sec() - global) <= crate::app::TICK_EOF_TAIL_SEC {
        return false;
    }
    let Some((next, next_global)) = bar.tl.next_chapter_after(&chapter) else {
        return false;
    };
    if crate::video_ext::paths_same_file(&next, &chapter) {
        return false;
    }
    crate::dvd_vob_log::dvd_seek_log(format!(
        "eof_advance: {} -> {} global={next_global:.2}",
        chapter.display(),
        next.display()
    ));
    let (_, local) = bar.resolve_global(next_global);
    if b.load_chapter_seek(&next, local, next_global, true).is_err() {
        b.dvd_hold_global.set(None);
        return false;
    }
    true
}
