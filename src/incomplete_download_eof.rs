//! Incomplete Direct Connect (`.dctmp`) at natural EOF: pause and stay put.
//! Demux `duration` often equals the downloaded prefix, so “near end” is not trustworthy —
//! while the path still uses the incomplete-download suffix, never auto-advance.
//! See `docs/features/07-sibling-folder-queue.md`.

use crate::human_media_title::is_incomplete_download_path;
use libmpv2::Mpv;
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

/// Session flag shared by sibling-EOF advance and Play/Pause unpause.
pub(crate) struct IncompleteEofHold {
    /// After unpause: skip immediate re-pause until EOF settles or keep-open pauses again.
    continue_armed: Cell<bool>,
}

impl IncompleteEofHold {
    pub(crate) fn new() -> Rc<Self> {
        Rc::new(Self {
            continue_armed: Cell::new(false),
        })
    }

    pub(crate) fn reset(&self) {
        self.continue_armed.set(false);
    }

    /// Pause a `.dctmp` at natural EOF instead of loading the next sibling.
    /// Returns `true` when the caller must not advance.
    pub(crate) fn hold_instead_of_advance(&self, mpv: &Mpv, path: &Path) -> bool {
        if !is_incomplete_download_path(path) {
            self.continue_armed.set(false);
            return false;
        }
        let eof = mpv.get_property::<bool>("eof-reached").unwrap_or(false);
        let paused = mpv.get_property::<bool>("pause").unwrap_or(false);
        if self.continue_armed.get() {
            if !eof {
                self.continue_armed.set(false);
            } else if paused {
                // keep-open paused again after a failed continue — next Play may re-arm.
                self.continue_armed.set(false);
            }
            return true;
        }
        if !paused {
            let _ = mpv.set_property("pause", true);
            eprintln!(
                "[rhino] dctmp: pause at EOF (still downloading) path={} pos={:.2} dur={:.2}",
                path.display(),
                mpv.get_property::<f64>("time-pos").unwrap_or(0.0),
                mpv.get_property::<f64>("duration").unwrap_or(0.0),
            );
        }
        true
    }

    /// Before clearing `pause`: arm continue and re-seek so demux can read newly grown bytes.
    pub(crate) fn on_unpause(&self, mpv: &Mpv, path: &Path) {
        if !is_incomplete_download_path(path) {
            return;
        }
        if !mpv.get_property::<bool>("eof-reached").unwrap_or(false) {
            return;
        }
        self.continue_armed.set(true);
        let pos = mpv
            .get_property::<f64>("time-pos")
            .ok()
            .filter(|p| p.is_finite())
            .unwrap_or(0.0)
            .max(0.0);
        let pos_s = format!("{pos}");
        match mpv.command("seek", &[&pos_s, "absolute+exact"]) {
            Ok(()) => eprintln!(
                "[rhino] dctmp: resume seek={pos:.2} path={}",
                path.display()
            ),
            Err(e) => eprintln!(
                "[rhino] dctmp: resume seek failed err={e:?} pos={pos:.2} path={}",
                path.display()
            ),
        }
    }
}
