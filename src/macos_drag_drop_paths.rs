// Pasteboard → local paths for Finder drops (`NSFilenamesPboardType` / file URLs).

use std::ffi::CStr;

use objc2::runtime::AnyObject;
use objc2::ClassType;
use objc2_app_kit::{
    NSPasteboard, NSPasteboardNameDrag, NSPasteboardType, NSPasteboardTypeFileURL,
};
use objc2_foundation::{NSArray, NSString, NSURL};

fn file_pasteboard_types() -> Retained<NSArray<NSPasteboardType>> {
    // Finder still offers legacy filenames; keep both.
    #[allow(deprecated)]
    let filenames = unsafe { objc2_app_kit::NSFilenamesPboardType };
    NSArray::from_slice(&[unsafe { NSPasteboardTypeFileURL }, filenames])
}

fn drag_pasteboard_offers_files() -> bool {
    let pb = NSPasteboard::pasteboardWithName(unsafe { NSPasteboardNameDrag });
    pb.availableTypeFromArray(&file_pasteboard_types()).is_some()
}

fn drag_op_for_info(info: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
    let pb = info.draggingPasteboard();
    if pb.availableTypeFromArray(&file_pasteboard_types()).is_some() {
        NSDragOperation::Copy
    } else {
        NSDragOperation::None
    }
}

fn path_from_nsurl(url: &NSURL) -> Option<PathBuf> {
    let ptr = url.fileSystemRepresentation();
    let cstr = unsafe { CStr::from_ptr(ptr.as_ptr()) };
    Some(PathBuf::from(cstr.to_string_lossy().as_ref()))
}

fn paths_from_filenames_plist(pb: &NSPasteboard) -> Vec<PathBuf> {
    #[allow(deprecated)]
    let filenames = unsafe { objc2_app_kit::NSFilenamesPboardType };
    let Some(any) = pb.propertyListForType(filenames) else {
        return Vec::new();
    };
    let Some(arr) = any.downcast_ref::<NSArray>() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|o: Retained<AnyObject>| {
            let s = o.downcast_ref::<NSString>()?;
            Some(PathBuf::from(s.to_string()))
        })
        .collect()
}

fn paths_from_file_urls(pb: &NSPasteboard) -> Vec<PathBuf> {
    let classes = NSArray::from_slice(&[NSURL::class()]);
    let Some(objs) = (unsafe { pb.readObjectsForClasses_options(&classes, None) }) else {
        return Vec::new();
    };
    objs.iter()
        .filter_map(|o: Retained<AnyObject>| {
            let url = o.downcast_ref::<NSURL>()?;
            path_from_nsurl(url)
        })
        .collect()
}

fn paths_from_info(info: &ProtocolObject<dyn NSDraggingInfo>) -> Vec<PathBuf> {
    let pb = info.draggingPasteboard();
    let mut paths = paths_from_filenames_plist(&pb);
    if paths.is_empty() {
        paths = paths_from_file_urls(&pb);
    }
    paths
}
