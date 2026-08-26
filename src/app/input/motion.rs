#[cfg(target_os = "macos")]
include!("motion_macos_unfocused.rs");

include!("motion_gl_area.rs");
include!("motion_window_reveal.rs");

/// True while this pointer sample falls inside a post-interaction squelch window.
fn motion_squelched(sq: &Rc<Cell<Option<Instant>>>) -> bool {
    sq.get().is_some_and(|t| Instant::now() < t)
}

/// True when this motion sample must be ignored: squelched, or same position as the last one
/// seen through `last`.
fn motion_sample_stale(
    sq: &Rc<Cell<Option<Instant>>>,
    last: &Rc<Cell<Option<(f64, f64)>>>,
    x: f64,
    y: f64,
) -> bool {
    motion_squelched(sq)
        || last
            .get()
            .is_some_and(|(lx, ly)| same_xy(x, lx) && same_xy(y, ly))
}

/// Reveal bars once per hide cycle (no-op while they are already shown).
fn reveal_bars_once<R: gtk::prelude::IsA<gtk::Widget>>(
    b: &Rc<Cell<bool>>,
    parts: ChromeApplyParts<'_, R>,
) {
    if b.get() {
        return;
    }
    b.set(true);
    apply_chrome(parts);
}
