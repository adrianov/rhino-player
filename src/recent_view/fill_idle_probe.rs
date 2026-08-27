/// DB-only cards on the main thread, then thumb backfill on the next idle (no libmpv).
pub fn fill_continue_strip(
    row: &gtk::Box,
    paths: Vec<std::path::PathBuf>,
    hooks: ContinueStripHooks,
    backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    schedule_backfill: BackfillFn,
) {
    let n = ensure_recent_backfill(&backfill, row, hooks);
    n.paint(paths.clone(), StripKind::ContinueList);
    glib::idle_add_local_once(move || schedule_backfill(n, paths));
}

