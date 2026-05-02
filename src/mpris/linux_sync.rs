//! Applies transport snapshots onto the session D-Bus MPRIS `Player` and handles seek signals.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use adw::prelude::FileExt;
use gtk::gio;
use mpris_server::{
    Metadata, PlaybackStatus, Player, Time, TrackId, zbus,
};

use super::MprisShot;

pub(super) enum MprisCtl {
    Sync(MprisShot),
    Seeked(Time),
}

pub(super) async fn dispatch_mpris_ctl(p: &Player, msg: MprisCtl) {
    match msg {
        MprisCtl::Sync(shot) => log_mpris_ctl("sync", apply_shot(p, &shot).await),
        MprisCtl::Seeked(pos) => log_mpris_ctl("seeked", p.seeked(pos).await),
    }
}

fn track_id_for_path(p: Option<&Path>) -> TrackId {
    let mut h = DefaultHasher::new();
    p.hash(&mut h);
    let id = h.finish();
    let s = format!("/ch/rhino/track{:x}", id);
    TrackId::try_from(s.as_str()).unwrap_or(TrackId::NO_TRACK)
}

fn file_uri_for_path(p: &Path) -> Option<String> {
    Some(gio::File::for_path(p).uri().to_string())
}

async fn apply_shot_stopped(p: &Player) -> zbus::Result<()> {
    p.set_playback_status(PlaybackStatus::Stopped).await?;
    p.set_metadata(Metadata::new()).await?;
    p.set_position(Time::ZERO);
    p.set_can_play(false).await?;
    p.set_can_pause(false).await?;
    p.set_can_seek(false).await?;
    p.set_can_go_next(false).await?;
    p.set_can_go_previous(false).await?;
    Ok(())
}

fn mpris_track_metadata(s: &MprisShot) -> Metadata {
    let tid = track_id_for_path(s.track_path.as_deref());
    let len_us = (s.dur_sec * 1_000_000.0).round().max(0.0) as i64;
    let mut meta = Metadata::builder()
        .trackid(tid)
        .length(Time::from_micros(len_us));
    if let Some(ref t) = s.title {
        meta = meta.title(t.clone());
    }
    if let Some(ref path) = s.track_path {
        if let Some(url) = file_uri_for_path(path) {
            meta = meta.url(url);
        }
    }
    meta.build()
}

async fn apply_shot_active(p: &Player, s: &MprisShot) -> zbus::Result<()> {
    let status = if s.paused {
        PlaybackStatus::Paused
    } else {
        PlaybackStatus::Playing
    };
    p.set_playback_status(status).await?;
    p.set_can_play(true).await?;
    p.set_can_pause(true).await?;
    p.set_can_seek(s.dur_sec > f64::EPSILON).await?;
    p.set_can_go_next(s.can_next).await?;
    p.set_can_go_previous(s.can_prev).await?;
    p.set_metadata(mpris_track_metadata(s)).await?;
    let pos_us = (s.pos_sec * 1_000_000.0).round() as i64;
    p.set_position(Time::from_micros(pos_us.clamp(0, i64::MAX)));
    Ok(())
}

async fn apply_shot(p: &Player, s: &MprisShot) -> zbus::Result<()> {
    if s.stopped {
        apply_shot_stopped(p).await
    } else {
        apply_shot_active(p, s).await
    }
}

fn log_mpris_ctl(label: &'static str, r: zbus::Result<()>) {
    if let Err(e) = r {
        eprintln!("[rhino] MPRIS {label}: {e}");
    }
}
