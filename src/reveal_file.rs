//! Reveal a local file in the platform file manager with that item selected (feature 38).

use std::path::Path;

/// Open the file manager on [path]'s folder and select the file. Logs failures to stderr.
///
/// Linux uses an async session-bus call so a slow file manager cannot stall the GTK loop.
pub fn reveal(path: &Path) {
    if let Err(e) = reveal_inner(path) {
        eprintln!("[rhino] reveal: {e} path={}", path.display());
    }
}

fn reveal_inner(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err("file missing".into());
    }
    let abs = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    platform_reveal(&abs)
}

#[cfg(target_os = "macos")]
fn platform_reveal(path: &Path) -> Result<(), String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSArray, NSURL};

    let url =
        NSURL::from_file_path(path).ok_or_else(|| "path not representable".to_string())?;
    NSWorkspace::sharedWorkspace()
        .activateFileViewerSelectingURLs(&NSArray::from_slice(&[url.as_ref()]));
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_reveal(path: &Path) -> Result<(), String> {
    let uri = glib::filename_to_uri(path, None).map_err(|e| e.to_string())?;
    show_items_uri_async(uri);
    Ok(())
}

/// Session-bus `ShowItems` without blocking the main thread (feature 38).
#[cfg(target_os = "linux")]
fn show_items_uri_async(uri: String) {
    use glib::prelude::ToVariant;

    gio::bus_get(
        gio::BusType::Session,
        gio::Cancellable::NONE,
        move |bus| match bus {
            Err(e) => eprintln!("[rhino] reveal: session bus: {e} uri={uri}"),
            Ok(conn) => call_show_items(conn, uri),
        },
    );
}

#[cfg(target_os = "linux")]
fn call_show_items(conn: gio::DBusConnection, uri: String) {
    use gio::prelude::*;
    use glib::prelude::ToVariant;

    conn.call(
        Some("org.freedesktop.FileManager1"),
        "/org/freedesktop/FileManager1",
        Some("org.freedesktop.FileManager1"),
        "ShowItems",
        Some(&glib::Variant::tuple_from_iter([
            [uri.as_str()].to_variant(),
            "".to_variant(),
        ])),
        None,
        gio::DBusCallFlags::NONE,
        30_000,
        gio::Cancellable::NONE,
        move |res| {
            if let Err(e) = res {
                eprintln!("[rhino] reveal: {e} uri={uri}");
            }
        },
    );
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_reveal(_path: &Path) -> Result<(), String> {
    Err("unsupported platform".into())
}

/// Hover-button tooltip for the current OS.
pub fn reveal_tooltip() -> &'static str {
    if cfg!(target_os = "macos") {
        "Reveal in Finder"
    } else {
        "Show in Files"
    }
}
