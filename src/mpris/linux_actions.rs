// D-Bus MPRIS action wiring (include!'d from `linux.rs`): player construction,
// window/app actions, transport controls, and seek/position handlers.

async fn build_mpris_player(suffix: &str) -> Option<Player> {
    match Player::builder(suffix)
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
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("[rhino] MPRIS: {e}");
            None
        }
    }
}

/// Clone-and-dispatch wiring for a no-argument transport action.
fn wire_simple_action(
    player: &Player,
    act: &std::rc::Rc<dyn Fn()>,
    connect: impl FnOnce(&Player, Box<dyn Fn(&Player)>),
) {
    let f = act.clone();
    connect(
        player,
        Box::new(move |_| {
            let f = f.clone();
            run_on_main(move || f());
        }),
    );
}

fn connect_window_actions(player: &Player, args: &MprisStartArgs) {
    let win = args.win.clone();
    player.connect_raise(move |_| {
        let win = win.clone();
        run_on_main(move || {
            win.present();
        });
    });

    let app = args.app.clone();
    player.connect_quit(move |_| {
        let app = app.clone();
        run_on_main(move || {
            app.quit();
        });
    });
}

fn connect_transport_controls(player: &Player, args: &MprisStartArgs) {
    wire_simple_action(player, &args.toggle_play_pause, |p, h| {
        p.connect_play_pause(h)
    });
    wire_simple_action(player, &args.unpause_only, |p, h| p.connect_play(h));
    wire_simple_action(player, &args.pause_only, |p, h| p.connect_pause(h));
    wire_simple_action(player, &args.stop, |p, h| p.connect_stop(h));
    wire_simple_action(player, &args.prev, |p, h| p.connect_previous(h));
    wire_simple_action(player, &args.next, |p, h| p.connect_next(h));
}

/// Borrow the active mpv bundle, if any, and apply `f` to it.
fn with_active_bundle<R>(
    cell: &std::rc::Rc<std::cell::RefCell<Option<MpvBundle>>>,
    f: impl FnOnce(&MpvBundle) -> R,
) -> Option<R> {
    let Ok(g) = cell.try_borrow() else {
        return None;
    };
    let b = g.as_ref()?;
    Some(f(b))
}

/// Shared seek dispatch: clone handles, then on the main thread borrow the active bundle,
/// map the current time-pos through `target_from_pos`, and seek + emit Seeked.
fn dispatch_position_seek(
    cell: &std::rc::Rc<std::cell::RefCell<Option<MpvBundle>>>,
    tx: &async_channel::Sender<MprisCtl>,
    seek_abs: &std::rc::Rc<dyn Fn(&str)>,
    target_from_pos: impl FnOnce(f64) -> f64 + 'static,
) {
    let cell = cell.clone();
    let tx = tx.clone();
    let seek_abs = seek_abs.clone();
    run_on_main(move || {
        with_active_bundle(&cell, |b| {
            seek_abs_and_emit_seeked(b, target_from_pos(bundle_time_pos_sec(b)), &seek_abs, &tx);
        });
    });
}

fn connect_relative_seek(
    player: &Player,
    args: &MprisStartArgs,
    tx: &async_channel::Sender<MprisCtl>,
) {
    let mpv_cell = args.mpv_bundle.clone();
    let tx = tx.clone();
    let seek_abs = args.seek_abs.0.clone();
    player.connect_seek(move |_, off| {
        let delta = off.as_micros() as f64 / 1_000_000.0;
        dispatch_position_seek(&mpv_cell, &tx, &seek_abs, move |pos| pos + delta);
    });
}

fn connect_absolute_position(
    player: &Player,
    args: &MprisStartArgs,
    tx: &async_channel::Sender<MprisCtl>,
) {
    let mpv_cell = args.mpv_bundle.clone();
    let tx = tx.clone();
    let seek_abs = args.seek_abs.0.clone();
    player.connect_set_position(move |_, _tid, position| {
        let sec = position.as_micros() as f64 / 1_000_000.0;
        dispatch_position_seek(&mpv_cell, &tx, &seek_abs, move |_| sec);
    });
}
