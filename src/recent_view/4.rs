/// Probes each path in an idle; [card_data_list] is DB-only (no libmpv) on the main thread, then
/// [schedule_backfill] starts missing-cache work when the owner decides it will not compete with startup.
pub fn fill_idle(
    row: &gtk::Box,
    paths: Vec<std::path::PathBuf>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    schedule_backfill: BackfillFn,
) {
    let row = row.clone();
    let o = on_open;
    let r = on_remove;
    let t = on_trash;
    let _ = glib::idle_add_local(move || {
        eprintln!(
            "[rhino] recent: fill_idle build grid for {} path(s):",
            paths.len()
        );
        for p in &paths {
            eprintln!("[rhino] recent:   candidate {}", p.display());
        }
        let n = ensure_recent_backfill(&backfill, &row, o.clone(), r.clone(), t.clone());
        let v: Vec<CardData> = card_data_list(&paths);
        eprintln!("[rhino] recent: card_data done ({} cards)", v.len());
        for cd in &v {
            eprintln!(
                "[rhino] recent:   card path={} missing={}",
                cd.path.display(),
                cd.missing
            );
        }
        fill_row(&row, v, o.clone(), r.clone(), t.clone());
        let paths_t = paths.clone();
        schedule_backfill(n, paths_t);
        glib::ControlFlow::Break
    });
}
