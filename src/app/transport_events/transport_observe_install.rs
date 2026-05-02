thread_local! {
    /// Coalesce one-shot GTK idle retries when [collect_events] cannot `borrow_mut` the player
    /// (e.g. UI code holds `borrow()` across `mpv.set_property("pause", …)`). Without a retry,
    /// libmpv events stay queued until the next wakeup and pause/unpause may skip redundant Smooth `vf` work.
    static TRANSPORT_DRAIN_RETRY_PENDING: Cell<bool> = const { Cell::new(false) };
}

/// Returns true once the bundle exists and observers are installed.
fn install_observers_when_ready(ctx: &Rc<TransportCtx>) -> bool {
    let trace = std::env::var_os("RHINO_TRANSPORT_TRACE").is_some();
    let mut g = match ctx.player.try_borrow_mut() {
        Ok(g) => g,
        Err(_) => {
            if trace {
                eprintln!("[rhino] transport install: player busy, deferring");
            }
            return false;
        }
    };
    let Some(b) = g.as_mut() else {
        if trace {
            eprintln!("[rhino] transport install: player not ready, deferring");
        }
        return false;
    };
    if let Err(e) = b.observe_props(&[
        (PROP_PAUSE, "pause", Format::Flag),
        (PROP_DURATION, "duration", Format::Double),
        (PROP_VOLUME, "volume", Format::Double),
        (PROP_MUTE, "mute", Format::Flag),
        (PROP_VOLUME_MAX, "volume-max", Format::Double),
        (PROP_PATH, "path", Format::String),
        (PROP_CONTAINER_FPS, "container-fps", Format::Double),
    ]) {
        eprintln!("[rhino] transport observe_props failed: {e}");
        return false;
    }
    let drain_ctx = ctx.clone();
    b.install_event_drain(move || drain_into_main(&drain_ctx));
    if trace {
        eprintln!("[rhino] transport install: observers wired, draining initial events");
    }
    drop(g);
    // Pull current state directly from mpv so the play / seek / nav UI is correct **right now**,
    // even if the warm-preloaded file finished loading before observers were registered.
    transport_tick(ctx);
    refresh_sibling_nav(ctx);
    drain_into_main(ctx);
    install_transport_tick(ctx);
    true
}

fn drain_into_main(ctx: &Rc<TransportCtx>) {
    let evs = collect_events(ctx);
    for e in evs {
        dispatch_event(ctx, e);
    }
}

fn schedule_transport_drain_retry(ctx: &Rc<TransportCtx>) {
    TRANSPORT_DRAIN_RETRY_PENDING.with(|p| {
        if p.replace(true) {
            return;
        }
        let c = Rc::clone(ctx);
        let _ = glib::idle_add_local_once(move || {
            TRANSPORT_DRAIN_RETRY_PENDING.with(|p| p.set(false));
            drain_into_main(&c);
        });
    });
}

fn collect_events(ctx: &Rc<TransportCtx>) -> Vec<TransportEv> {
    let mut out: Vec<TransportEv> = Vec::new();
    let mut g = match ctx.player.try_borrow_mut() {
        Ok(g) => g,
        Err(_) => {
            schedule_transport_drain_retry(ctx);
            return out;
        }
    };
    let Some(b) = g.as_mut() else {
        return out;
    };
    b.drain_events(|ev| match ev {
        Event::PropertyChange {
            reply_userdata, change, ..
        } => {
            if let Some(t) = property_event(reply_userdata, change) {
                out.push(t);
            }
        }
        Event::FileLoaded => out.push(TransportEv::FileLoaded),
        Event::VideoReconfig => out.push(TransportEv::VideoReconfig),
        _ => {}
    });
    out
}

fn property_event(id: u64, data: PropertyData<'_>) -> Option<TransportEv> {
    Some(match (id, &data) {
        (PROP_PAUSE, PropertyData::Flag(v)) => TransportEv::Pause(*v),
        (PROP_DURATION, PropertyData::Double(v)) => TransportEv::Duration(*v),
        (PROP_VOLUME, PropertyData::Double(v)) => TransportEv::Volume(*v),
        (PROP_MUTE, PropertyData::Flag(v)) => TransportEv::Mute(*v),
        (PROP_VOLUME_MAX, PropertyData::Double(v)) => TransportEv::VolumeMax(*v),
        (PROP_PATH, PropertyData::Str(_)) => TransportEv::PathChanged,
        (PROP_CONTAINER_FPS, PropertyData::Double(_)) => TransportEv::ContainerFpsChanged,
        _ => return None,
    })
}
