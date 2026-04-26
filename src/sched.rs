//! Best-effort CPU + (Linux) I/O priority so decode and the UI are less likely to stutter under load.
//! - **Unix:** [setpriority](https://man7.org/linux/man-pages/man2/getpriority.2.html) for the whole
//!   process (libmpv and GTK share it). Unprivileged users may be capped by `RLIMIT_NICE` — failure
//!   is ignored.
//! - **Linux only:** [ioprio_set](https://man7.org/linux/man-pages/man2/ioprio_set.2.html) for
//!   best-effort I/O, highest level within the class (same idea as `ionice -c2 -n0`).

/// Target nice: lower = higher CPU priority. Often achievable without `CAP_SYS_NICE` with default
/// `rlimit` (distro-dependent); if not, the call no-ops.
#[cfg(unix)]
const NICE_TARGET: i32 = -8;

/// Linux `ioprio` value: [IOPRIO_CLASS_BE](https://docs.kernel.org/block/ioprio.html) + highest level
/// (0 = best within class).
#[cfg(target_os = "linux")]
const IOPRIO_WHO_PROCESS: libc::c_long = 1;

#[cfg(target_os = "linux")]
fn raise_io_for_playback_linux() {
    const CLASS_SHIFT: i32 = 13;
    const CLASS_BE: i32 = 2;
    // BFQ / mq-deadline: class in high bits, best-effort level 0 in low bits.
    let ioprio: libc::c_long = (CLASS_BE << CLASS_SHIFT) as libc::c_long;
    unsafe {
        let _ = libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, 0, ioprio);
    }
}

/// Call at process start (before GLib / GTK) so the whole app prefers CPU + disk service.
#[cfg(unix)]
pub fn raise_process_priority() {
    unsafe {
        let _ = libc::setpriority(libc::PRIO_PROCESS, 0, NICE_TARGET);
    }
    #[cfg(target_os = "linux")]
    raise_io_for_playback_linux();
}

#[cfg(not(unix))]
pub fn raise_process_priority() {}
