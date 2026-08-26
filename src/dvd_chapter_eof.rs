// DVD mid-title chapter EOF: detect tail of open `.vob` and load the next chapter.

use libmpv2::Mpv;

include!("dvd_chapter_eof_detect.rs");

impl DvdVobTimeline {
    /// Map mpv EOF on the open `.vob` to the next `loadfile` target and whole-title hold time.
    ///
    /// PTT chapter marks and on-disk `.vob` splits rarely align; always derive the target from
    /// live tail `time-pos` / `duration`, not the stored start of the next file in the bar.
    pub fn continue_after_vob_eof(
        &self,
        current: &Path,
        local_eof: f64,
    ) -> Option<(PathBuf, f64, f64)> {
        let i = self.index_of(current)?;
        if i + 1 >= self.vobs.len() {
            return None;
        }
        let g_cont = self.continuation_global(i, local_eof);
        let (idx, local) = self.resolve_global(g_cont);
        let target = self.vobs.get(idx)?.clone();
        if crate::video_ext::paths_same_file(&target, current) {
            return self.same_file_fallback(i, self.starts[i] + local_eof.max(0.0), g_cont);
        }
        Some((target, local, g_cont))
    }

    /// Global continuation seconds: just past the live tail, clamped into the title length.
    fn continuation_global(&self, i: usize, local_eof: f64) -> f64 {
        (self.starts[i] + local_eof.max(0.0) + 0.05).min(self.total_sec)
    }

    /// Next `.vob` when the continuation point resolves back into the current file:
    /// resume at the following part, honoring the stored boundary only past its end.
    fn same_file_fallback(&self, i: usize, g_eof: f64, g_cont: f64) -> Option<(PathBuf, f64, f64)> {
        let j = i + 1;
        let target = self.vobs[j].clone();
        let stored_end = self.starts[i] + self.durs[i].max(0.0);
        let mut local = if g_eof + 1e-3 >= stored_end {
            (g_cont - self.starts[j]).max(0.0)
        } else {
            0.0
        };
        if self.durs[j] > 0.0 {
            local = local.min((self.durs[j] - 0.05).max(0.0));
        }
        Some((target, local, g_cont))
    }
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
    match plan_chapter_eof_load(b, bar) {
        Some(plan) => load_next_chapter(b, plan),
        None => false,
    }
}

/// Resolved next-chapter `loadfile`: source, target, resume position and hold times.
struct EofAdvancePlan {
    chapter: PathBuf,
    next: PathBuf,
    local: f64,
    hold_global: f64,
    local_eof: f64,
}

/// Resolve the next-chapter `loadfile` target once the open `.vob` reaches its tail.
fn plan_chapter_eof_load(
    b: &crate::mpv_embed::MpvBundle,
    bar: &DvdBarState,
) -> Option<EofAdvancePlan> {
    let chapter = bundle_open_chapter(b)?;
    if !chapter_local_at_eof_for(&b.mpv, Some(chapter.as_path()), Some(&bar.tl)) {
        return None;
    }
    if !clear_stale_chapter_scrub(b) {
        return None;
    }
    let local_eof = chapter_eof_local_sec(&b.mpv, &chapter, &bar.tl);
    let (next, local, hold_global) = next_target_after_tail(bar, &chapter, local_eof)?;
    Some(EofAdvancePlan {
        chapter,
        next,
        local,
        hold_global,
        local_eof,
    })
}

/// Chapter path currently open in the playing bundle.
fn bundle_open_chapter(b: &crate::mpv_embed::MpvBundle) -> Option<PathBuf> {
    let shell = b.me_budget_shell_path.borrow().clone();
    open_dvd_chapter_path(&b.mpv, shell.as_deref())
}

/// Next segment past the live tail; logs when no further segment exists.
fn next_target_after_tail(
    bar: &DvdBarState,
    chapter: &Path,
    local_eof: f64,
) -> Option<(PathBuf, f64, f64)> {
    let Some((next, local, hold_global)) = bar.tl.continue_after_vob_eof(chapter, local_eof) else {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "eof_advance: no next segment after {} local={local_eof:.2}",
            chapter.display()
        ));
        return None;
    };
    (!crate::video_ext::paths_same_file(&next, chapter)).then_some((next, local, hold_global))
}

/// Load the planned next chapter, logging progress and any rejected `loadfile`.
fn load_next_chapter(b: &crate::mpv_embed::MpvBundle, plan: EofAdvancePlan) -> bool {
    crate::dvd_vob_log::dvd_seek_log(format!(
        "eof_advance: {} -> {} global={:.2} local={:.2} (tail={:.2})",
        plan.chapter.display(),
        plan.next.display(),
        plan.hold_global,
        plan.local,
        plan.local_eof
    ));
    if b.load_chapter_seek(&plan.next, plan.local, plan.hold_global, true, true)
        .is_err()
    {
        eprintln!(
            "[rhino] dvd: eof_advance loadfile failed {} -> {}",
            plan.chapter.display(),
            plan.next.display()
        );
        return false;
    }
    true
}

/// Clear a stale chapter-scrub load before advancing; `false` = do not advance.
fn clear_stale_chapter_scrub(b: &crate::mpv_embed::MpvBundle) -> bool {
    if !b.chapter_cross_load_busy() {
        return true;
    }
    if b.chapter_scrub_resume_pending() {
        return false;
    }
    crate::dvd_vob_log::dvd_seek_log("eof_advance: clear stale chapter scrub");
    b.abort_chapter_load(true);
    true
}
