//! Best-effort process nice value so playback is less likely to stutter under CPU load.
//! On Linux this uses [setpriority](https://man7.org/linux/man-man/man2/getpriority.2.html)
//! for the current process; unprivileged users may be capped by `RLIMIT_NICE` — failure is ignored.

/// Target nice: lower = higher CPU priority. `-5` is a common media tunable without `CAP_SYS_NICE`.
#[cfg(unix)]
const NICE_TARGET: i32 = -5;

/// Raise this process’s scheduling priority when the OS allows it (e.g. `RLIMIT_NICE` ≥ 5).
#[cfg(unix)]
pub fn raise_process_priority() {
    unsafe {
        // whole-process; libmpv runs in the same process as GTK.
        let _ = libc::setpriority(libc::PRIO_PROCESS, 0, NICE_TARGET);
    }
}

#[cfg(not(unix))]
pub fn raise_process_priority() {}
