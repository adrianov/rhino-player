//! Session-bus MPRIS2 service: `org.mpris.MediaPlayer2` + Player.
//! Runs on the GLib main context together with GTK; control messages arrive on an async channel.

use std::sync::Mutex;

use adw::prelude::{ApplicationExt, GtkWindowExt};
use futures::future::join;
use gtk::glib;
use mpris_server::{Player, Time};

use crate::mpv_embed::MpvBundle;
use crate::APP_ID;
use super::linux_sync::{dispatch_mpris_ctl, MprisCtl};
use super::{MprisShot, MprisStartArgs};

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

pub(crate) fn start_linux(args: MprisStartArgs) {
    let suffix = format!("RhinoPlayer_{}", std::process::id());
    let app = args.app.clone();
    let win = args.win.clone();
    let mpv = args.mpv_bundle.clone();
    let seek_abs = args.seek_abs.0.clone();
    let toggle = args.toggle_play_pause;
    let pause_only = args.pause_only;
    let unpause = args.unpause_only;
    let stop = args.stop;
    let prev = args.prev;
    let next = args.next;

    glib::spawn_future_local(async move {
        let player = match Player::builder(suffix.as_str())
            .can_quit(true)
            .can_raise(true)
            .identity("Rhino Player")
            .desktop_entry(APP_ID)
            .can_play(false)
            .can_pause(false)
            .can_seek(false)
            .can_go_next(false)
            .can_go_previous(false)
            .build()
            .await
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[rhino] MPRIS: {e}");
                return;
            }
        };

        let (tx, rx) = async_channel::bounded::<MprisCtl>(32);
        {
            let Ok(mut g) = MPRIS_TX.lock() else {
                return;
            };
            *g = Some(tx.clone());
        }

        player.connect_raise(move |_| {
            let w = win.clone();
            run_on_main(move || {
                w.present();
            });
        });

        player.connect_quit(move |_| {
            let a = app.clone();
            run_on_main(move || {
                a.quit();
            });
        });

        player.connect_play_pause(move |_| {
            let f = toggle.clone();
            run_on_main(move || {
                f();
            });
        });

        player.connect_play(move |_| {
            let f = unpause.clone();
            run_on_main(move || {
                f();
            });
        });

        player.connect_pause(move |_| {
            let f = pause_only.clone();
            run_on_main(move || {
                f();
            });
        });

        player.connect_stop(move |_| {
            let f = stop.clone();
            run_on_main(move || {
                f();
            });
        });

        player.connect_previous(move |_| {
            let f = prev.clone();
            run_on_main(move || {
                f();
            });
        });

        player.connect_next(move |_| {
            let f = next.clone();
            run_on_main(move || {
                f();
            });
        });

        let mpv_seek = mpv.clone();
        let tx_seek = tx.clone();
        let seek_fn = seek_abs.clone();
        player.connect_seek(move |_, off| {
            let p = mpv_seek.clone();
            let t = tx_seek.clone();
            let sf = seek_fn.clone();
            run_on_main(move || {
                let Ok(g) = p.try_borrow() else {
                    return;
                };
                let Some(b) = g.as_ref() else {
                    return;
                };
                let delta = off.as_micros() as f64 / 1_000_000.0;
                let nt = bundle_time_pos_sec(b) + delta;
                seek_abs_and_emit_seeked(b, nt, &sf, &t);
            });
        });

        let mpv_set = mpv.clone();
        let tx_sp = tx.clone();
        let seek_fn_sp = seek_abs.clone();
        player.connect_set_position(move |_, _tid, position| {
            let p = mpv_set.clone();
            let t = tx_sp.clone();
            let sf = seek_fn_sp.clone();
            run_on_main(move || {
                let Ok(g) = p.try_borrow() else {
                    return;
                };
                let Some(b) = g.as_ref() else {
                    return;
                };
                let sec = position.as_micros() as f64 / 1_000_000.0;
                seek_abs_and_emit_seeked(b, sec, &sf, &t);
            });
        });

        let run_task = player.run();
        let ctl_loop = async {
            while let Ok(msg) = rx.recv().await {
                dispatch_mpris_ctl(&player, msg).await;
            }
        };

        join(run_task, ctl_loop).await;
    });
}
