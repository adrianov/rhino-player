/// Smooth `vf` reattach policy around pause -> unpause transitions.
enum ReattachNeed {
    Yes,
    No,
    /// Pause(false) arrived while [TransportCtx::player] was already borrowed (load / drain).
    BorrowBusy,
}

/// Unpause needs a Smooth resync only when the graph is missing / was stripped (Smooth on),
/// or a stale graph must be removed (Smooth off). Plain pause→resume with a live graph skips it.
fn smooth_needs_reattach_on_unpause(ctx: &Rc<TransportCtx>) -> ReattachNeed {
    // try_borrow: pause events may be dispatched while the bundle is already borrowed.
    let Ok(g) = ctx.player.try_borrow() else {
        return ReattachNeed::BorrowBusy;
    };
    let Some(b) = g.as_ref() else {
        return ReattachNeed::No;
    };
    if !has_open_path(&b.mpv) {
        return ReattachNeed::No;
    }
    let has_vf = crate::video_pref::vf_chain_has_vapoursynth(&b.mpv);
    if !ctx.video_pref.borrow().smooth_60 {
        return if has_vf {
            ReattachNeed::Yes
        } else {
            ReattachNeed::No
        };
    }
    if b.smooth_vf_stripped_this_open() || !has_vf {
        ReattachNeed::Yes
    } else {
        ReattachNeed::No
    }
}

fn sync_smooth_vf_on_pause_transition(ctx: &Rc<TransportCtx>, paused: bool) {
    if !paused {
        match smooth_needs_reattach_on_unpause(ctx) {
            ReattachNeed::Yes => schedule_smooth_60_resync_idle(ctx),
            ReattachNeed::BorrowBusy if ctx.video_pref.borrow().smooth_60 => {
                let c = Rc::clone(ctx);
                glib::idle_add_local_once(move || {
                    if matches!(smooth_needs_reattach_on_unpause(&c), ReattachNeed::Yes) {
                        schedule_smooth_60_resync_idle(&c);
                    }
                });
            }
            _ => {}
        }
    }
    ctx.eof.gl.queue_render();
}
