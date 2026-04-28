struct VideoFileActions {
    close_video: gio::SimpleAction,
    move_to_trash: gio::SimpleAction,
}

struct VideoFileActionCtx {
    app: adw::Application,
    player: Rc<RefCell<Option<MpvBundle>>>,
    recent: gtk::ScrolledWindow,
    /// Single closure replacing repeated [BackToBrowseCtx] construction; arg = `clear_undo`.
    on_browse_back: Rc<dyn Fn(bool)>,
    undo_timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
    do_commit: Rc<dyn Fn() + 'static>,
    close_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    trash_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
}
