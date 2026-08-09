// Temporary Smooth **vf** pause when overload meets **external** machine load.
// Player-heavy overload still lowers ME budget via **`smooth_budget_transport_apply`**.
// (No `use` imports — `include!`d into `video_pref` before `smooth_budget`.)

/// Cooldown before retrying Smooth after an external-load pause.
const LOAD_HOLD_SECS: u64 = 60;

/// 1‑minute load average per logical CPU above this ⇒ the machine is busy.
const EXTERNAL_LOADAVG_PER_CPU: f64 = 0.85;

/// Viewer share below this (of all logical CPUs) ⇒ other apps dominate when loadavg is high.
const SELF_CPU_NOT_DOMINANT_FRAC: f64 = 0.55;

thread_local! {
    static LOAD_HOLD_UNTIL: std::cell::Cell<Option<std::time::Instant>> =
        const { std::cell::Cell::new(None) };
}

#[must_use]
pub(crate) fn smooth_load_hold_active() -> bool {
    // Stays armed until resume / pref-off — wall clock alone does not clear (resume is on the tick).
    LOAD_HOLD_UNTIL.with(|c| c.get().is_some())
}

pub(crate) fn smooth_load_hold_clear() {
    LOAD_HOLD_UNTIL.with(|c| c.set(None));
}

#[must_use]
pub(crate) fn smooth_load_hold_tooltip() -> &'static str {
    "Smooth Video paused — CPU busy with other apps. Tries again in about a minute."
}

/// Pure gate used by tests and [`external_load_contention`].
#[must_use]
pub(crate) fn external_load_contention_at(
    load_per_cpu: Option<f64>,
    process_cpu_frac: Option<f64>,
) -> bool {
    let Some(load) = load_per_cpu else {
        return false;
    };
    if load < EXTERNAL_LOADAVG_PER_CPU {
        return false;
    }
    process_cpu_frac.is_some_and(|self_f| self_f < SELF_CPU_NOT_DOMINANT_FRAC)
}

/// Machine busy **and** this process is not the main consumer.
#[must_use]
pub(crate) fn external_load_contention(process_cpu_frac: Option<f64>) -> bool {
    external_load_contention_at(loadavg_per_cpu(), process_cpu_frac)
}

fn loadavg_per_cpu() -> Option<f64> {
    #[cfg(unix)]
    {
        let mut av = [0.0_f64; 3];
        let n = unsafe { libc::getloadavg(av.as_mut_ptr(), 3) };
        if n < 1 {
            return None;
        }
        let cpus = std::thread::available_parallelism()
            .map(|x| x.get() as f64)
            .unwrap_or(1.0)
            .max(1.0);
        Some(av[0] / cpus)
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn fmt_cpu_frac(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.3}")).unwrap_or_else(|| "n/a".into())
}

fn arm_hold_until(from: std::time::Instant) {
    LOAD_HOLD_UNTIL.with(|c| {
        c.set(Some(
            from + std::time::Duration::from_secs(LOAD_HOLD_SECS),
        ));
    });
}

/// Start a pause: unload **vf**, keep preference on, leave ME budget unchanged.
pub(crate) fn enter_smooth_load_hold(
    player: &std::rc::Rc<std::cell::RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    process_cpu_frac: Option<f64>,
) {
    arm_hold_until(std::time::Instant::now());
    eprintln!(
        "[rhino] smooth: decision load_hold secs={LOAD_HOLD_SECS} loadavg_per_cpu={} process_cpu_frac={} (external load — motion budget unchanged)",
        fmt_cpu_frac(loadavg_per_cpu()),
        fmt_cpu_frac(process_cpu_frac),
    );
    if let Ok(g) = player.try_borrow() {
        if let Some(b) = g.as_ref() {
            strip_vapoursynth_before_replace_media(b);
        }
    }
}

/// While paused for load: skip budget adaptation. After about a minute, restore **vf**
/// (overload may pause again if other apps are still saturating the machine).
/// Returns **true** when Smooth is still paused for load or was just restored this tick.
pub(crate) fn smooth_load_hold_on_tick(
    player: &std::rc::Rc<std::cell::RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    video_pref: &std::rc::Rc<std::cell::RefCell<crate::db::VideoPrefs>>,
) -> bool {
    if !video_pref.borrow().smooth_60 {
        smooth_load_hold_clear();
        return false;
    }
    let Some(until) = LOAD_HOLD_UNTIL.with(|c| c.get()) else {
        return false;
    };
    if std::time::Instant::now() < until {
        return true;
    }
    smooth_load_hold_clear();
    eprintln!("[rhino] smooth: decision load_hold_resume (pause elapsed — restoring Smooth)");
    let mut vp = video_pref.borrow_mut();
    let _ = apply_mpv_video(player, &mut vp, None);
    true
}

#[cfg(test)]
mod load_hold_tests {
    use super::*;

    #[test]
    fn player_dominated_load_does_not_count_as_external() {
        assert!(!external_load_contention_at(Some(1.5), Some(0.80)));
        assert!(!external_load_contention_at(Some(1.5), Some(0.55)));
    }

    #[test]
    fn other_process_load_counts_when_self_is_modest() {
        assert!(external_load_contention_at(Some(0.90), Some(0.40)));
        assert!(external_load_contention_at(Some(1.20), Some(0.10)));
    }

    #[test]
    fn quiet_machine_never_external() {
        assert!(!external_load_contention_at(Some(0.20), Some(0.10)));
        assert!(!external_load_contention_at(None, Some(0.10)));
        assert!(!external_load_contention_at(Some(0.90), None));
    }

    #[test]
    fn hold_arm_and_clear() {
        smooth_load_hold_clear();
        assert!(!smooth_load_hold_active());
        arm_hold_until(std::time::Instant::now());
        assert!(smooth_load_hold_active());
        smooth_load_hold_clear();
        assert!(!smooth_load_hold_active());
    }
}
