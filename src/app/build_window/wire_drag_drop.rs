// Main-window drag/drop: Linux uses `GtkDropTargetAsync`; macOS uses AppKit
// `NSDraggingDestination` (`macos_drag_drop`) because gdk-macos GTK drops are unreliable.

#[cfg(not(target_os = "macos"))]
use gio::prelude::InputStreamExtManual;

#[cfg(not(target_os = "macos"))]
include!("wire_drag_drop_codec.rs");
#[cfg(not(target_os = "macos"))]
include!("wire_drag_drop_sink.rs");
include!("wire_drag_drop_open.rs");
#[cfg(not(target_os = "macos"))]
include!("wire_drag_drop_read.rs");

fn wire_window_drop_targets(
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub_menu: &gtk::MenuButton,
    on_open: &RcPathFn,
) {
    #[cfg(target_os = "macos")]
    {
        let player = Rc::clone(player);
        let sub_menu = sub_menu.clone();
        let on_open = Rc::clone(on_open);
        crate::macos_drag_drop::install(win, move |paths| {
            consume_dropped_paths(paths, &player, &sub_menu, &on_open);
        });
    }

    #[cfg(not(target_os = "macos"))]
    wire_gtk_window_drop_targets(win, player, sub_menu, on_open);
}

#[cfg(not(target_os = "macos"))]
fn wire_gtk_window_drop_targets(
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub_menu: &gtk::MenuButton,
    on_open: &RcPathFn,
) {
    let merged = drag_dest_formats_union();

    let tgt = gtk::DropTargetAsync::builder()
        .actions(gtk::gdk::DragAction::COPY)
        .formats(&merged)
        .propagation_phase(gtk::PropagationPhase::Capture)
        .build();

    tgt.connect_drag_enter(|_, _drop, _x, _y| gtk::gdk::DragAction::COPY);
    tgt.connect_drag_motion(|_, _drop, _x, _y| gtk::gdk::DragAction::COPY);

    connect_drop_accept(&tgt, &merged);
    connect_window_drop(&tgt, merged.clone(), player, sub_menu, on_open);

    win.add_controller(tgt);
}

#[cfg(not(target_os = "macos"))]
fn connect_drop_accept(tgt: &gtk::DropTargetAsync, merged: &gtk::gdk::ContentFormats) {
    let fm_accept = merged.clone();
    tgt.connect_accept(move |_t, dk_drop| {
        dk_drop.formats().match_(&fm_accept)
            || dk_drop
                .formats()
                .contains_type(gtk::gdk::FileList::static_type())
            || dk_drop.formats().contains_type(gio::File::static_type())
            || !mime_types_ordered_for_drop_read(dk_drop).is_empty()
    });
}

#[cfg(not(target_os = "macos"))]
fn connect_window_drop(
    tgt: &gtk::DropTargetAsync,
    fm_types: gtk::gdk::ContentFormats,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub_menu: &gtk::MenuButton,
    on_open: &RcPathFn,
) {
    let player = Rc::clone(player);
    let sub_menu = sub_menu.clone();
    let on_open = Rc::clone(on_open);
    tgt.connect_drop(move |_t, dk, _, _| {
        let negotiated_ok = dk.formats().match_type(&fm_types).is_valid();
        if !negotiated_ok && drop_lacks_readable_type(dk) {
            eprintln!("[rhino] dnd: reject drop (no negotiated type / MIME / file list)");
            return false;
        }
        try_read_drop_async(
            dk.clone(),
            fm_types.clone(),
            Rc::clone(&player),
            sub_menu.clone(),
            Rc::clone(&on_open),
        );
        true
    });
}

#[cfg(not(target_os = "macos"))]
fn drop_lacks_readable_type(dk: &gtk::gdk::Drop) -> bool {
    mime_types_ordered_for_drop_read(dk).is_empty()
        && !dk
            .formats()
            .contains_type(gtk::gdk::FileList::static_type())
        && !dk.formats().contains_type(gio::File::static_type())
}
