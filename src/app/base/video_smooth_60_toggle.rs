// Smooth 60 menu action state handling. Split out of video_menu_and_filters.rs
// (include!'d into the same module scope).

/// Main menu: [db::VideoPrefs] and `app.*` actions for `gio::Menu` (before [win::present]).
fn handle_smooth_apply_result(
    app: &adw::Application,
    pl: &Rc<RefCell<Option<MpvBundle>>>,
    r: video_pref::MpvVideoApply,
) {
    if !r.smooth_auto_off {
        return;
    }
    let vf_still_on = pl
        .borrow()
        .as_ref()
        .is_some_and(|b| crate::video_pref::vf_chain_has_vapoursynth(&b.mpv));
    if vf_still_on {
        eprintln!(
            "[rhino] video: vf add error ignored — vapoursynth still active; keeping Smooth on"
        );
        return;
    }
    sync_smooth_60_to_off(app);
    show_smooth_setup_dialog(app);
}

struct Smooth60ToggleCtx {
    app: adw::Application,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl_area: gtk::GLArea,
    smooth_lbl: Option<gtk::Label>,
    smooth_btn: Option<gtk::Button>,
}

fn sync_smooth_60_programmatic(
    action: &gio::SimpleAction,
    value: &glib::Variant,
    b: bool,
    ctx: &Smooth60ToggleCtx,
) {
    crate::user_action_log::act(format!("smooth-60 menu sync (programmatic) -> {b}"));
    action.set_state(value);
    sync_smooth_toolbar_on(ctx.smooth_btn.as_ref(), b);
    stamp_smooth_toolbar_readout(
        ctx.smooth_lbl.as_ref(),
        ctx.smooth_btn.as_ref(),
        &ctx.player,
    );
}

/// mvtools missing: force Smooth off, persist, and surface the setup dialog.
fn reject_smooth_60_no_mvtools(action: &gio::SimpleAction, ctx: &Smooth60ToggleCtx) {
    let mut g = ctx.video_pref.borrow_mut();
    g.smooth_60 = false;
    db::save_video(&g);
    action.set_state(&false.to_variant());
    sync_smooth_toolbar_on(ctx.smooth_btn.as_ref(), false);
    show_smooth_setup_dialog(&ctx.app);
    ctx.gl_area.queue_render();
    stamp_smooth_toolbar_readout(
        ctx.smooth_lbl.as_ref(),
        ctx.smooth_btn.as_ref(),
        &ctx.player,
    );
}

/// Player already loaded: apply the vf swap (or reset + reload when enabling while playing).
fn apply_smooth_60_while_playing(ctx: &Smooth60ToggleCtx, b: bool) {
    let reload = {
        let mut g = ctx.video_pref.borrow_mut();
        let reload = b && video_pref::smooth_user_enable_playing_reset(&ctx.player, &mut g);
        if !reload {
            // Off→on (or post-seek strip): arm exact playhead resync for reattach only.
            let stripped = b
                && ctx
                    .player
                    .borrow()
                    .as_ref()
                    .is_some_and(MpvBundle::smooth_vf_stripped_this_open);
            if stripped {
                video_pref::arm_smooth_reattach_av_resync();
            }
            let r = video_pref::apply_mpv_video(&ctx.player, &mut g, None);
            video_pref::clear_smooth_reattach_av_resync();
            handle_smooth_apply_result(&ctx.app, &ctx.player, r);
        }
        reload
    };
    // Release pause-hold if a playing Smooth-on reload was aborted (e.g. toggle off mid-reload).
    if !reload {
        if let Some(bundle) = ctx.player.borrow().as_ref() {
            video_pref::maybe_unpause_after_smooth_reload(&bundle.mpv);
        }
    }
}

fn on_smooth_60_change_state(
    action: &gio::SimpleAction,
    value: &glib::Variant,
    ctx: &Smooth60ToggleCtx,
) {
    let Some(b) = value.get::<bool>() else {
        return;
    };
    if smooth_60_action_programmatic() {
        sync_smooth_60_programmatic(action, value, b, ctx);
        return;
    }
    crate::user_action_log::act(format!(
        "smooth-60 menu -> {}",
        if b { "on" } else { "off" }
    ));
    if b {
        crate::app::cancel_smooth_60_transport_resync();
        crate::video_pref::cancel_deferred_vf_swap();
    }
    if b && !can_find_mvtools(&ctx.video_pref.borrow()) {
        reject_smooth_60_no_mvtools(action, ctx);
        return;
    }
    commit_smooth_60_state(action, value, b, ctx);
}

/// User-driven state accepted: persist the preference and resync player + toolbar chrome.
fn commit_smooth_60_state(
    action: &gio::SimpleAction,
    value: &glib::Variant,
    b: bool,
    ctx: &Smooth60ToggleCtx,
) {
    action.set_state(value);
    sync_smooth_toolbar_on(ctx.smooth_btn.as_ref(), b);
    {
        let mut g = ctx.video_pref.borrow_mut();
        g.smooth_60 = b;
        db::save_video(&g);
    }
    if ctx.player.borrow().as_ref().is_some() {
        apply_smooth_60_while_playing(ctx, b);
    }
    ctx.gl_area.queue_render();
    stamp_smooth_toolbar_readout(
        ctx.smooth_lbl.as_ref(),
        ctx.smooth_btn.as_ref(),
        &ctx.player,
    );
}
