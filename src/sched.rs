//! **Linux** only: set the process **I/O scheduling class** to best-effort with the highest
//! priority *within that class* via [ioprio_set] (see [I/O priorities](https://docs.kernel.org/block/ioprio.html);
//! same idea as `ionice -c2 -n0` with BFQ / mq-deadline). This is a real **class**-based knob for
//! block I/O, not [setpriority] **niceness** (which is often limited by `RLIMIT_NICE` in sandboxes
//! and is not a separate “class” in the I/O path).
//!
//! We do not adjust CPU niceness. Other platforms: no-op.

/// `IOPRIO_WHO_PROCESS` and `scheduling class` in high bits, level in low bits.
#[cfg(target_os = "linux")]
const IOPRIO_WHO_PROCESS: libc::c_long = 1;

/// [IOPRIO_CLASS_BE](https://docs.kernel.org/block/ioprio.html) (2) with level 0 (best in class).
#[cfg(target_os = "linux")]
fn apply_io_class_best_effort() {
    const CLASS_SHIFT: i32 = 13;
    const CLASS_BE: i32 = 2;
    let ioprio: libc::c_long = (CLASS_BE << CLASS_SHIFT) as libc::c_long;
    unsafe {
        let _ = libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, 0, ioprio);
    }
}

/// Call at process start, before GLib / GTK, so disk reads (decode) stay smooth under I/O load.
#[cfg(target_os = "linux")]
pub fn raise_process_priority() {
    apply_io_class_best_effort();
}

#[cfg(all(unix, not(target_os = "linux")))]
pub fn raise_process_priority() {}

#[cfg(not(unix))]
pub fn raise_process_priority() {}
