//! macOS **Open Video** sheet: `NSOpenPanel` with real UTIs (GTK maps mime types to
//! internal `dyn.*` ids that do not enable AVCHD/BDMV “Media Collection” packages).

use std::cell::RefCell;
use std::ffi::CStr;
use std::path::PathBuf;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSModalResponseOK, NSOpenPanel, NSWindow};
use objc2_foundation::{NSArray, NSString, NSURL};

use crate::video_ext;

thread_local! {
    static OPEN_PICK: RefCell<Option<Box<dyn FnOnce(Option<PathBuf>)>>> = const { RefCell::new(None) };
}

fn panel_allowed_file_types(mtm: MainThreadMarker) -> objc2::rc::Retained<NSArray<NSString>> {
    let mut types: Vec<objc2::rc::Retained<NSString>> = Vec::new();
    for uti in [
        "public.movie",
        "public.avchd-collection",
        "public.avchd-content",
        "public.folder",
    ] {
        types.push(NSString::from_str(uti));
    }
    for ext in video_ext::SUFFIX {
        types.push(NSString::from_str(ext));
    }
    for ext in ["bdmv", "bdm"] {
        types.push(NSString::from_str(ext));
    }
    let _ = mtm;
    NSArray::from_retained_slice(&types)
}

fn path_from_url(url: &objc2::rc::Retained<NSURL>) -> Option<PathBuf> {
    let ptr = url.fileSystemRepresentation();
    let cstr = unsafe { CStr::from_ptr(ptr.as_ptr()) };
    Some(PathBuf::from(cstr.to_string_lossy().as_ref()))
}

/// Presents the native open sheet; `on_pick` runs on the GTK main loop (may be `None`).
pub fn present_open_video_sheet(
    parent: &adw::ApplicationWindow,
    on_pick: impl FnOnce(Option<PathBuf>) + 'static,
) -> bool {
    let Some(_mtm) = MainThreadMarker::new() else {
        eprintln!("[rhino] open video: NSOpenPanel requires the main thread");
        return false;
    };
    let Some(ns_win) = crate::macos_window::nswindow_for_widget(parent) else {
        eprintln!("[rhino] open video: no NSWindow for parent");
        return false;
    };
    OPEN_PICK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(on_pick));
    });
    present_open_video_sheet_ns(ns_win);
    true
}

fn present_open_video_sheet_ns(ns_win: objc2::rc::Retained<NSWindow>) {
    let mtm = MainThreadMarker::new().expect("main thread");
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setTitle(Some(&NSString::from_str("Open Video")));
    panel.setPrompt(Some(&NSString::from_str("Open")));
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(false);
    panel.setTreatsFilePackagesAsDirectories(false);
    panel.setAllowedFileTypes(Some(&panel_allowed_file_types(mtm)));

    let panel_ret = panel.clone();
    let handler = RcBlock::new(move |response| {
        let path = (response == NSModalResponseOK)
            .then(|| panel_ret.URL())
            .flatten()
            .as_ref()
            .and_then(path_from_url);
        glib::idle_add_local_once(move || {
            let pick = OPEN_PICK.with(|slot| slot.borrow_mut().take());
            if let Some(f) = pick {
                f(path);
            }
        });
    });
    panel.beginSheetModalForWindow_completionHandler(&ns_win, &handler);
}
