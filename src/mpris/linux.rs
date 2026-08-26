//! Session-bus MPRIS2 service: `org.mpris.MediaPlayer2` + Player.
//! Runs on the GLib main context together with GTK; control messages arrive on an async channel.

use std::sync::Mutex;

use adw::prelude::{ApplicationExt, GtkWindowExt};
use futures::future::join;
use gtk::glib;
use mpris_server::{Player, Time};

use super::linux_sync::{dispatch_mpris_ctl, MprisCtl};
use super::{MprisShot, MprisStartArgs};
use crate::mpv_embed::MpvBundle;
use crate::APP_ID;

static MPRIS_TX: Mutex<Option<async_channel::Sender<MprisCtl>>> = Mutex::new(None);

fn bundle_duration_sec(b: &MpvBundle) -> f64 {
    let dur = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    if dur.is_finite() {
        dur.max(0.0)
    } else {
        0.0
    }
}

fn bundle_time_pos_sec(b: &MpvBundle) -> f64 {
    let pos = b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    if pos.is_finite() {
        pos.max(0.0)
    } else {
        0.0
    }
}

fn seek_abs_and_emit_seeked(
    b: &MpvBundle,
    target_sec: f64,
    seek_abs: &std::rc::Rc<dyn Fn(&str)>,
    tx: &async_channel::Sender<MprisCtl>,
) {
    let dur = bundle_duration_sec(b);
    if dur <= f64::EPSILON {
        return;
    }
    let nt = target_sec.clamp(0.0, dur);
    let s = format!("{nt:.4}");
    seek_abs(&s);
    let _ = tx.try_send(MprisCtl::Seeked(Time::from_micros(
        (nt * 1_000_000.0).round() as i64,
    )));
}

pub(crate) fn enqueue_snapshot(shot: MprisShot) {
    let Ok(g) = MPRIS_TX.lock() else {
        return;
    };
    let Some(tx) = g.as_ref() else {
        return;
    };
    let _ = tx.try_send(MprisCtl::Sync(shot));
}

fn run_on_main(f: impl FnOnce() + 'static) {
    let mut slot = Some(f);
    glib::idle_add_local(move || {
        if let Some(task) = slot.take() {
            task();
        }
        glib::ControlFlow::Break
    });
}

/// Fresh bounded control channel; its sender becomes the global snapshot target.
fn open_ctl_channel() -> Option<(
    async_channel::Sender<MprisCtl>,
    async_channel::Receiver<MprisCtl>,
)> {
    let (tx, rx) = async_channel::bounded::<MprisCtl>(32);
    let mut g = MPRIS_TX.lock().ok()?;
    *g = Some(tx.clone());
    Some((tx, rx))
}

pub(crate) fn start_linux(args: MprisStartArgs) {
    let suffix = format!("RhinoPlayer_{}", std::process::id());

    glib::spawn_future_local(async move {
        let Some(player) = build_mpris_player(&suffix).await else {
            return;
        };
        let Some((tx, rx)) = open_ctl_channel() else {
            return;
        };

        connect_window_actions(&player, &args);
        connect_transport_controls(&player, &args);
        connect_relative_seek(&player, &args, &tx);
        connect_absolute_position(&player, &args, &tx);

        let run_task = player.run();
        let ctl_loop = async {
            while let Ok(msg) = rx.recv().await {
                dispatch_mpris_ctl(&player, msg).await;
            }
        };

        join(run_task, ctl_loop).await;
    });
}

include!("linux_actions.rs");
