#[cfg(not(target_os = "macos"))]
include!("wire_drag_drop_fallback.rs");

fn read_value_type(
    dk_read: gtk::gdk::Drop,
    dk_finish: gtk::gdk::Drop,
    typ: glib::types::Type,
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_menu: gtk::MenuButton,
    on_open: RcPathFn,
    on_empty: impl FnOnce() + 'static,
) {
    dk_read.read_value_async(
        typ,
        glib::Priority::default(),
        None::<&gio::Cancellable>,
        move |got| match got {
            Ok(val) => {
                let paths = paths_from_gvalue(val.type_(), &val);
                if !paths.is_empty() {
                    dispatch_paths_and_finish_drop(paths, &player, &sub_menu, &on_open, &dk_finish);
                } else {
                    on_empty();
                }
            }
            Err(_) => {
                on_empty();
            }
        },
    );
}
