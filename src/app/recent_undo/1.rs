struct RecentUndoWiring {
    recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    pending_recent_backfill: Rc<RefCell<Option<RecentBackfillJob>>>,
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
    undo_timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    do_commit: Rc<dyn Fn() + 'static>,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
}

struct RecentUndoCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    recent: gtk::ScrolledWindow,
    flow: gtk::Box,
    undo_shell: gtk::Box,
    undo_label: gtk::Label,
    undo_btn: gtk::Button,
    undo_close: gtk::Button,
    on_open: RcPathFn,
    want_recent: bool,
}
