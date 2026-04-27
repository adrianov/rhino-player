struct VideoFileActions {
    close_video: gio::SimpleAction,
    move_to_trash: gio::SimpleAction,
}

struct VideoFileActionCtx {
    app: adw::Application,
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
    recent: gtk::ScrolledWindow,
    flow_recent: gtk::Box,
    gl: gtk::GLArea,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    sibling_nav: SiblingNavUi,
    browse_chrome: Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    undo_shell: gtk::Box,
    undo_label: gtk::Label,
    undo_btn: gtk::Button,
    undo_timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
    do_commit: Rc<dyn Fn() + 'static>,
    close_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    trash_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
}

