//! Finder → main window file drops via AppKit [`NSDraggingDestination`].
//!
//! GTK4 `DropTarget` on gdk-macos often misses drops (especially when the window is
//! not focused). A transparent `NSView` overlay registers for file pasteboard types,
//! participates in hit-testing only while the drag pasteboard offers files, and
//! otherwise returns null so GTK keeps normal clicks and gestures.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::WidgetExt;
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::{define_class, msg_send, MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSDragOperation, NSDraggingDestination, NSDraggingInfo, NSView,
};
use objc2_foundation::NSObjectProtocol;

use crate::macos_window::nswindow_for_widget;

include!("macos_drag_drop_paths.rs");

type PathsHandler = Rc<dyn Fn(Vec<PathBuf>)>;

thread_local! {
    static HANDLER: RefCell<Option<PathsHandler>> = const { RefCell::new(None) };
}

struct DropIvars;

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "RhinoDropView"]
    #[ivars = DropIvars]
    struct RhinoDropView;

    unsafe impl NSObjectProtocol for RhinoDropView {}

    impl RhinoDropView {
        /// Only claim the hit when Finder (or similar) is dragging files; otherwise
        /// return null so gdk-macos receives normal pointer events on `contentView`.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: objc2_foundation::NSPoint) -> *mut NSView {
            if !drag_pasteboard_offers_files() {
                return std::ptr::null_mut();
            }
            Retained::autorelease_return(self.retain().into_super())
        }
    }

    unsafe impl NSDraggingDestination for RhinoDropView {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(
            &self,
            sender: &ProtocolObject<dyn NSDraggingInfo>,
        ) -> NSDragOperation {
            drag_op_for_info(sender)
        }

        #[unsafe(method(draggingUpdated:))]
        fn dragging_updated(
            &self,
            sender: &ProtocolObject<dyn NSDraggingInfo>,
        ) -> NSDragOperation {
            drag_op_for_info(sender)
        }

        #[unsafe(method(prepareForDragOperation:))]
        fn prepare_for_drag_operation(
            &self,
            sender: &ProtocolObject<dyn NSDraggingInfo>,
        ) -> Bool {
            Bool::new(!paths_from_info(sender).is_empty())
        }

        #[unsafe(method(performDragOperation:))]
        fn perform_drag_operation(
            &self,
            sender: &ProtocolObject<dyn NSDraggingInfo>,
        ) -> Bool {
            let paths = paths_from_info(sender);
            if paths.is_empty() {
                eprintln!("[rhino] dnd: macOS drop had no local file paths");
                return Bool::new(false);
            }
            let n = paths.len();
            glib::idle_add_local_once(move || {
                HANDLER.with(|slot| {
                    if let Some(h) = slot.borrow().clone() {
                        h(paths);
                    } else {
                        eprintln!("[rhino] dnd: macOS drop with no handler ({n} path(s))");
                    }
                });
            });
            Bool::new(true)
        }
    }
);

fn attach_drop_view(win: &adw::ApplicationWindow) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("[rhino] dnd: NSDraggingDestination requires the main thread");
        return false;
    };
    let Some(nswin) = nswindow_for_widget(win) else {
        return false;
    };
    let content: *mut NSView = unsafe { msg_send![&*nswin, contentView] };
    let Some(content) = (unsafe { Retained::retain(content) }) else {
        eprintln!("[rhino] dnd: NSWindow has no contentView");
        return false;
    };

    let this = RhinoDropView::alloc(mtm).set_ivars(DropIvars);
    let view: Retained<RhinoDropView> = unsafe { msg_send![super(this), init] };
    let bounds = content.bounds();
    view.setFrame(bounds);
    view.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    view.registerForDraggedTypes(&file_pasteboard_types());
    content.addSubview(&view);
    true
}

fn try_attach_once(win: &adw::ApplicationWindow, attached: &Cell<bool>) {
    if attached.get() {
        return;
    }
    if attach_drop_view(win) {
        attached.set(true);
    }
}

/// Install Finder drop handling on `win`. Safe to call before realize; attaches on map.
pub fn install(win: &adw::ApplicationWindow, on_paths: impl Fn(Vec<PathBuf>) + 'static) {
    HANDLER.with(|slot| {
        *slot.borrow_mut() = Some(Rc::new(on_paths));
    });
    let attached = Rc::new(Cell::new(false));
    if win.is_realized() {
        try_attach_once(win, &attached);
    }
    let attached_rz = Rc::clone(&attached);
    win.connect_realize(move |w| try_attach_once(w, &attached_rz));
    // Present can realize without a second realize signal in some gdk-macos paths.
    let win2 = win.clone();
    let attached_idle = Rc::clone(&attached);
    glib::idle_add_local_once(move || {
        try_attach_once(&win2, &attached_idle);
    });
}
