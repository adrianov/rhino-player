/// DB-only cards on the main thread, then thumb backfill on the next idle (no libmpv).
pub fn fill_continue_strip(
    row: &gtk::Box,
    paths: Vec<std::path::PathBuf>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    warm_hover: Option<WarmHoverHooks>,
    backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    schedule_backfill: BackfillFn,
) {
    let n = ensure_recent_backfill(
        &backfill,
        row,
        on_open.clone(),
        on_remove.clone(),
        on_trash.clone(),
        warm_hover.clone(),
    );
    let v: Vec<CardData> = card_data_list(&paths);
    fill_row(
        row,
        v,
        on_open,
        on_remove,
        on_trash,
        warm_hover.as_ref(),
    );
    glib::idle_add_local_once(move || schedule_backfill(n, paths));
}

