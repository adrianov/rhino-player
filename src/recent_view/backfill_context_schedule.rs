/// Card actions and shared chrome cache for continue-strip cards.
pub struct ContinueStripHooks {
    pub on_open: RcPathFn,
    pub on_remove: RcPathFn,
    pub on_trash: RcPathFn,
    pub warm_hover: Option<WarmHoverHooks>,
    pub chrome_cache: crate::media_probe::ContinueGridCache,
    /// Neighbour-search state (feature 33); bound to the context at first spawn.
    pub search: Option<Rc<SiblingSearchState>>,
}

/// Fresh [RecentContext]. Thumb ready-paths are push-delivered by [ThumbBackfill] in live_card.
fn spawn_recent_context(row: &gtk::Box, hooks: ContinueStripHooks) -> Rc<RecentContext> {
    let ctx = Rc::new(RecentContext::from_hooks(row, hooks));
    ctx.finish_spawn();
    ctx
}

/// Creates or reuses a [RecentContext] in [cell] (one per window).
pub fn ensure_recent_backfill(
    cell: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    row: &gtk::Box,
    hooks: ContinueStripHooks,
) -> Rc<RecentContext> {
    if let Some(c) = cell.borrow().as_ref() {
        return Rc::clone(c);
    }
    let ctx = spawn_recent_context(row, hooks);
    *cell.borrow_mut() = Some(Rc::clone(&ctx));
    ctx
}

/// Ensure the strip context, then paint the query-aware strip and arm thumb workers.
/// Single entry for remove/undo/browse-back so search hits get the same backfill as continue.
pub fn ensure_apply_strip(
    cell: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    row: &gtk::Box,
    hooks: ContinueStripHooks,
) {
    ensure_recent_backfill(cell, row, hooks).apply_strip();
}

/// Hooks for a strip paint: warm-hover and neighbour-search ride along with the last context.
pub fn strip_hooks_from_cell(
    cell: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    chrome_cache: crate::media_probe::ContinueGridCache,
) -> ContinueStripHooks {
    let ctx = cell.borrow();
    ContinueStripHooks {
        on_open,
        on_remove,
        on_trash,
        warm_hover: ctx.as_ref().and_then(|c| c.warm_hover().cloned()),
        chrome_cache,
        search: ctx.as_ref().and_then(|c| c.search.as_ref().map(Rc::clone)),
    }
}

include!("backfill_context_schedule/card_pointer.rs");
