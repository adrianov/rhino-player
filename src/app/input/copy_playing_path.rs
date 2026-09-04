// Copy-file shortcut helpers (included from `input.rs`). Spec: `docs/features/13-input-shortcuts.md`.
// Places the open media as a filesystem item (Finder / Nautilus paste), not plain path text.

#[cfg(target_os = "macos")]
fn copy_path_modifier_held(m: gtk::gdk::ModifierType) -> bool {
    m.contains(gtk::gdk::ModifierType::META_MASK)
}

#[cfg(not(target_os = "macos"))]
fn copy_path_modifier_held(m: gtk::gdk::ModifierType) -> bool {
    m.contains(gtk::gdk::ModifierType::CONTROL_MASK)
}

fn try_copy_playing_path(player: &Rc<RefCell<Option<MpvBundle>>>) -> bool {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        eprintln!("[rhino] copy-file: no player");
        return false;
    };
    let shell = b.me_budget_shell_path.borrow();
    let Some(path) = crate::media_probe::shell_media_path(&b.mpv, shell.as_deref()) else {
        eprintln!("[rhino] copy-file: no open media path");
        return false;
    };
    if !path.exists() {
        eprintln!("[rhino] copy-file: path missing {}", path.display());
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        clipboard_put_file_macos(&path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        clipboard_put_file_gdk(&path)
    }
}

#[cfg(target_os = "macos")]
fn nspasteboard_file_url(path: &Path) -> Option<objc2::rc::Retained<objc2_foundation::NSURL>> {
    use objc2_foundation::NSURL;
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if abs.is_dir() {
        NSURL::from_directory_path(&abs)
    } else {
        NSURL::from_file_path(&abs)
    }
}

/// General pasteboard file URL — same representation Finder uses for ⌘C on a file/folder.
#[cfg(target_os = "macos")]
fn clipboard_put_file_macos(path: &Path) -> bool {
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{NSPasteboard, NSPasteboardWriting};
    use objc2_foundation::NSArray;

    let Some(url) = nspasteboard_file_url(path) else {
        eprintln!("[rhino] copy-file: path not representable {}", path.display());
        return false;
    };
    // Release any GTK clipboard owner first; otherwise gdk-macos can reclaim and wipe the
    // Finder file payload after we write the general pasteboard.
    if let Some(display) = gtk::gdk::Display::default() {
        let _ = display.clipboard().set_content(None::<&gtk::gdk::ContentProvider>);
    }
    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    if !pb.writeObjects(&NSArray::from_retained_slice(&[ProtocolObject::<
        dyn NSPasteboardWriting,
    >::from_retained(url)])) {
        eprintln!("[rhino] copy-file: NSPasteboard writeObjects failed path={}", path.display());
        return false;
    }
    crate::user_action_log::act(format!("key copy-file {}", path.display()));
    true
}

/// `GdkFileList` → `text/uri-list` for Nautilus and other GTK file managers.
#[cfg(not(target_os = "macos"))]
fn clipboard_put_file_gdk(path: &Path) -> bool {
    use glib::prelude::ToValue;

    let Some(display) = gtk::gdk::Display::default() else {
        eprintln!("[rhino] copy-file: no display");
        return false;
    };
    if let Err(e) = display
        .clipboard()
        .set_content(Some(&gtk::gdk::ContentProvider::for_value(
            &gtk::gdk::FileList::from_array(&[gio::File::for_path(path)]).to_value(),
        ))) {
        eprintln!("[rhino] copy-file: set_content failed path={} err={e}", path.display());
        return false;
    }
    crate::user_action_log::act(format!("key copy-file {}", path.display()));
    true
}
