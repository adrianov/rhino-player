// Coalesce resize notifies and ignore resize-end while a programmatic snap is applying.

thread_local! {
    static LAST_NOTIFY: std::cell::Cell<(i32, i32)> = const { std::cell::Cell::new((0, 0)) };
    static SNAP_PENDING: std::cell::Cell<Option<(i32, i32)>> = const { std::cell::Cell::new(None) };
}

fn near(a: i32, b: i32) -> bool {
    (a - b).abs() <= 1
}

fn near_size(a: (i32, i32), b: (i32, i32)) -> bool {
    near(a.0, b.0) && near(a.1, b.1)
}

/// Call before `apply_window_outer_size` when the shell will change outer dimensions.
pub(crate) fn note_programmatic_win_resize(nw: i32, nh: i32) {
    SNAP_PENDING.set(Some((nw, nh)));
}

/// True when width/height notify should run (skips duplicate width+height/surface pairs).
pub(crate) fn resize_notify_changed(ww: i32, hh: i32) -> bool {
    let last = LAST_NOTIFY.get();
    if last == (ww, hh) {
        return false;
    }
    LAST_NOTIFY.set((ww, hh));
    true
}

/// True when resize-end should not snap (programmatic resize in flight or just landed).
pub(crate) fn skip_resize_end_snap(ww: i32, hh: i32, vw: i64, vh: i64) -> bool {
    let Some(target) = SNAP_PENDING.get() else {
        return false;
    };
    if near_size((ww, hh), target) {
        SNAP_PENDING.set(None);
        return true;
    }
    if snap_size_after_user_resize(ww, hh, vw, vh) == Some(target) {
        return true;
    }
    SNAP_PENDING.set(None);
    false
}
