//! Trash via **Finder** ([`NSWorkspace::recycleURLs`]), so items appear in the Dock Trash.
//! [`gio::File::trash`] and [`NSFileManager::trashItemAtURL`] only rename into `~/.Trash`.

use block2::RcBlock;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSArray, NSDictionary, NSError, NSURL};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const RECYCLE_WAIT: Duration = Duration::from_secs(30);

/// Move file at [path] into Finder Trash; returns the path Finder used (for Undo).
///
/// Canonicalizes relative paths before building [`NSURL`] (required by Foundation).
pub fn move_to_trash_ns(path: &Path) -> Result<PathBuf, String> {
    let abs = std::fs::canonicalize(path).map_err(|e| format!("trash: {e}"))?;
    let url =
        NSURL::from_file_path(&abs).ok_or_else(|| "trash: path not representable".to_string())?;
    recycle_via_finder(&url)
}

fn recycle_via_finder(url: &NSURL) -> Result<PathBuf, String> {
    let urls = NSArray::from_slice(&[url]);
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |dict, err| {
        let _ = tx.send(recycle_result(dict, err));
        glib::MainContext::default().wakeup();
    });
    NSWorkspace::sharedWorkspace().recycleURLs_completionHandler(&urls, Some(&block));
    wait_recycle(rx)
}

fn recycle_result(
    new_urls: NonNull<NSDictionary<NSURL, NSURL>>,
    error: *mut NSError,
) -> Result<PathBuf, String> {
    if let Some(err) = unsafe { error.as_ref() } {
        return Err(err.localizedDescription().to_string());
    }
    let dict = unsafe { new_urls.as_ref() };
    let Some(trashed) = dict.allValues().firstObject() else {
        return Err("trash: Finder did not return a Trash location".into());
    };
    trashed
        .to_file_path()
        .ok_or_else(|| "trash: could not read trashed file path".into())
}

/// Pump the GTK loop while waiting: `recycleURLs` may invoke its handler on the main queue.
fn wait_recycle(rx: mpsc::Receiver<Result<PathBuf, String>>) -> Result<PathBuf, String> {
    let ctx = glib::MainContext::default();
    let deadline = Instant::now() + RECYCLE_WAIT;
    while Instant::now() < deadline {
        match rx.try_recv() {
            Ok(r) => return r,
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("trash: Finder recycle cancelled".into());
            }
            Err(mpsc::TryRecvError::Empty) => {
                if !ctx.iteration(false) {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
    eprintln!("[rhino] trash: Finder recycle timed out");
    Err("trash: Finder recycle timed out".into())
}
