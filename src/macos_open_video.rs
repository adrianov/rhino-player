//! macOS **Open Video** sheet: `NSOpenPanel` with real UTIs (GTK maps mime types to
//! internal `dyn.*` ids that do not enable AVCHD/BDMV “Media Collection” packages).

use std::cell::RefCell;
use std::ffi::CStr;
use std::path::PathBuf;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSModalResponse, NSModalResponseOK, NSOpenPanel, NSWindow};
use objc2_foundation::{NSArray, NSString, NSURL};
use objc2_uniform_type_identifiers::UTType;

use crate::video_ext;

/// Callback invoked with the picked path (or [None] when the sheet is cancelled).
type OpenPickFn = Box<dyn FnOnce(Option<PathBuf>)>;
thread_local! {
    static OPEN_PICK: RefCell<Option<OpenPickFn>> = const { RefCell::new(None) };
}

fn push_uti_id(types: &mut Vec<objc2::rc::Retained<UTType>>, id: &str) {
    if let Some(t) = UTType::typeWithIdentifier(&NSString::from_str(id)) {
        types.push(t);
    }
}

fn push_filename_ext(types: &mut Vec<objc2::rc::Retained<UTType>>, ext: &str) {
    if let Some(t) = UTType::typeWithFilenameExtension(&NSString::from_str(ext)) {
        types.push(t);
    }
}

fn panel_allowed_content_types() -> objc2::rc::Retained<NSArray<UTType>> {
    let mut types: Vec<objc2::rc::Retained<UTType>> = Vec::new();
    for uti in [
        "public.movie",
        "public.folder",
        "public.avchd-collection",
        "public.avchd-content",
        "public.mpeg",
        "jp.co.dvdfllc.vob",
    ] {
        push_uti_id(&mut types, uti);
    }
    for ext in video_ext::SUFFIX {
        push_filename_ext(&mut types, ext);
    }
    for ext in ["bdmv", "bdm", "ifo"] {
        push_filename_ext(&mut types, ext);
    }
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

fn configure_open_panel(panel: &NSOpenPanel) {
    panel.setTitle(Some(&NSString::from_str("Open Video")));
    panel.setPrompt(Some(&NSString::from_str("Open")));
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(false);
    panel.setTreatsFilePackagesAsDirectories(false);
    panel.setAllowedContentTypes(&panel_allowed_content_types());
}

fn finish_open_panel_response(panel: &NSOpenPanel, response: NSModalResponse) {
    let path = (response == NSModalResponseOK)
        .then(|| panel.URL())
        .flatten()
        .as_ref()
        .and_then(path_from_url);
    glib::idle_add_local_once(move || {
        if let Some(f) = OPEN_PICK.with(|slot| slot.borrow_mut().take()) {
            f(path);
        }
    });
}

fn present_open_video_sheet_ns(ns_win: objc2::rc::Retained<NSWindow>) {
    let panel = NSOpenPanel::openPanel(MainThreadMarker::new().expect("main thread"));
    configure_open_panel(&panel);
    let panel_ret = panel.clone();
    panel.beginSheetModalForWindow_completionHandler(
        &ns_win,
        &RcBlock::new(move |response| finish_open_panel_response(&panel_ret, response)),
    );
}
