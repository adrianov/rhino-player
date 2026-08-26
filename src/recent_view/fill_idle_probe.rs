/// DB-only cards on the main thread, then thumb backfill on the next idle (no libmpv).
pub fn fill_continue_strip(
    row: &gtk::Box,
    paths: Vec<std::path::PathBuf>,
    hooks: ContinueStripHooks,
    backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    schedule_backfill: BackfillFn,
) {
    let n = ensure_recent_backfill(
        &backfill,
        row,
        ContinueStripHooks {
            on_open: hooks.on_open.clone(),
            on_remove: hooks.on_remove.clone(),
            on_trash: hooks.on_trash.clone(),
            warm_hover: hooks.warm_hover.clone(),
            chrome_cache: Rc::clone(&hooks.chrome_cache),
        },
    );
    let v: Vec<CardData> = card_data_list(&paths);
    let ContinueStripHooks {
        on_open,
        on_remove,
        on_trash,
        warm_hover,
        chrome_cache,
    } = hooks;
    fill_row(
        row,
        v,
        on_open,
        on_remove,
        on_trash,
        warm_hover.as_ref(),
        Some(&chrome_cache),
    );
    glib::idle_add_local_once(move || schedule_backfill(n, paths));
}
