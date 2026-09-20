//! Bar-probe wiring: kick/resume cropdetect runs and the DB cache round-trip.

use super::FillSync;
use crate::black_bars::{pump_bar_probe, schedule_bar_probe, BarState, CropRect};
use crate::mpv_embed::MpvBundle;
use crate::video_fill::current_local_media_path;
use std::cell::RefCell;
use std::rc::Rc;

type Player = Rc<RefCell<Option<MpvBundle>>>;

impl FillSync {
    pub(super) fn kick_bar_probe(&self) {
        if self.restore_cached_bars() {
            crate::video_fill::request_fill_sync_only();
            return;
        }
        self.kick_bar_probe_live();
    }

    /// Always run cropdetect (skip DB cache) — used after Bob attaches late.
    pub(super) fn kick_bar_probe_live(&self) {
        schedule_bar_probe(
            &self.player,
            &self.bars,
            probe_done_cb(&self.player, &self.bars),
        );
    }

    pub(super) fn resume_bar_probe(&self, forced: bool) {
        pump_bar_probe(
            &self.player,
            &self.bars,
            probe_done_cb(&self.player, &self.bars),
            forced,
        );
    }

    /// Reuse `media.bar_crop` when the file mtime still matches.
    fn restore_cached_bars(&self) -> bool {
        let Some(path) = current_local_media_path(&self.player) else {
            return false;
        };
        let Some(cached) = crate::db::media_bar_crop(&path) else {
            return false;
        };
        let state = match cached.crop.as_deref() {
            None => {
                eprintln!("[rhino] bars: cached clean path={}", path.display());
                BarState::Clean
            }
            Some(spec) => match CropRect::parse_video_crop(spec) {
                Some(rect) => {
                    eprintln!(
                        "[rhino] bars: cached crop={} deint={} path={}",
                        rect.as_video_crop(),
                        cached.saw_deint,
                        path.display()
                    );
                    BarState::Crop(rect)
                }
                None => {
                    eprintln!(
                        "[rhino] bars: bad cached crop={spec:?} path={}",
                        path.display()
                    );
                    return false;
                }
            },
        };
        self.bars.restore_cached(state, cached.saw_deint);
        true
    }
}

fn probe_done_cb(player: &Player, bars: &Rc<crate::black_bars::BarProbe>) -> Rc<dyn Fn()> {
    let player = Rc::clone(player);
    let bars = Rc::clone(bars);
    Rc::new(move || {
        persist_bar_probe(&player, &bars);
        crate::video_fill::request_fill_sync_only();
    })
}

fn persist_bar_probe(player: &Player, bars: &Rc<crate::black_bars::BarProbe>) {
    let Some(path) = current_local_media_path(player) else {
        return;
    };
    let deint = bars.saw_deint();
    match bars.state.get() {
        BarState::Clean => crate::db::media_save_bar_crop(&path, None, deint),
        BarState::Crop(rect) => {
            crate::db::media_save_bar_crop(&path, Some(&rect.as_video_crop()), deint);
        }
        BarState::Unknown | BarState::Pending => {}
    }
}
