/// Boot / ensure paint for the continue strip (feature 21 / 33).
/// Same query-aware path as search and I'm Feeling Lucky: [ensure_apply_strip].
pub fn fill_continue_strip(
    row: &gtk::Box,
    hooks: ContinueStripHooks,
    backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
) {
    ensure_apply_strip(&backfill, row, hooks);
}
