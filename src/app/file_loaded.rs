struct FileLoadedCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    sibling_nav: SiblingNavUi,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    gl: gtk::GLArea,
    bar_show: Rc<Cell<bool>>,
    recent: gtk::Box,
    bottom: gtk::Box,
    sub_menu: gtk::MenuButton,
    close_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    trash_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    speed_sync: Rc<Cell<bool>>,
    speed_menu: gtk::MenuButton,
    speed_list: gtk::ListBox,
    speed_readout: gtk::Label,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
    close_video_btn: gtk::Button,
}

fn make_file_loaded_handler(ctx: FileLoadedCtx) -> Rc<dyn Fn()> {
    Rc::new(move || {
        let cur = ctx.last_path.borrow().clone();
        ctx.sibling_nav
            .refresh(cur.as_deref(), ctx.sibling_seof.as_ref());
        let t = tick_captures(&ctx);
        let _ = glib::timeout_add_local(Duration::from_millis(320), move || {
            on_320ms_tick(t.clone());
            glib::ControlFlow::Break
        });
        // 60p: load idle attaches vf; this 320 ms hook aligns speed env without racing it.
    })
}

/// Clones the pieces the one-shot 320 ms tick needs out of the file-loaded context.
fn tick_captures(ctx: &FileLoadedCtx) -> On320Ctx {
    On320Ctx {
        player: ctx.player.clone(),
        sub_pref: ctx.sub_pref.clone(),
        recent: ctx.recent.clone(),
        bar_show: ctx.bar_show.clone(),
        bottom: ctx.bottom.clone(),
        gl: ctx.gl.clone(),
        sub_btn: ctx.sub_menu.clone(),
        speed_sync_flag: ctx.speed_sync.clone(),
        speed_menu: ctx.speed_menu.clone(),
        speed_list: ctx.speed_list.clone(),
        speed_readout: ctx.speed_readout.clone(),
        video_pref: ctx.video_pref.clone(),
        app: ctx.app.clone(),
        close_action: ctx.close_action_cell.clone(),
        trash_action: ctx.trash_action_cell.clone(),
        close_video_btn: ctx.close_video_btn.clone(),
    }
}

include!("file_loaded/sub_style.rs");

#[derive(Clone)]
struct On320Ctx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    recent: gtk::Box,
    bar_show: Rc<Cell<bool>>,
    bottom: gtk::Box,
    gl: gtk::GLArea,
    sub_btn: gtk::MenuButton,
    speed_sync_flag: Rc<Cell<bool>>,
    speed_menu: gtk::MenuButton,
    speed_list: gtk::ListBox,
    speed_readout: gtk::Label,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
    close_action: Rc<RefCell<Option<gio::SimpleAction>>>,
    trash_action: Rc<RefCell<Option<gio::SimpleAction>>>,
    close_video_btn: gtk::Button,
}

fn on_320ms_tick(c: On320Ctx) {
    if let Some(b) = c.player.borrow().as_ref() {
        tick_player_side(&c, b);
    }
    if let Some(a) = c.close_action.borrow().as_ref() {
        sync_close_video_action(a, &c.close_video_btn, &c.player, &c.recent);
    }
    if let Some(a) = c.trash_action.borrow().as_ref() {
        sync_trash_action(a, &c.player, &c.recent);
    }
}

fn tick_player_side(c: &On320Ctx, b: &MpvBundle) {
    sync_sub_button_after_load(c.player.clone(), c.sub_btn.clone());
    let pr = c.sub_pref.borrow();
    // Pick `sid` before styling so BDMV text tracks get `sub-color` / `sub-scale`.
    sub_tracks::reapply_saved_or_autopick(&b.mpv, &pr, b.me_budget_shell_path.borrow().as_deref());
    sub_prefs::apply_mpv(&b.mpv, &pr);
    let show = c.recent.is_visible() || c.bar_show.get();
    sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, c.bottom.height(), c.gl.height());
    resync_speed_if_smooth_60(c, b);
}

fn resync_speed_if_smooth_60(c: &On320Ctx, b: &MpvBundle) {
    let listed = playback_speed::sync_list(
        &b.mpv,
        &c.speed_sync_flag,
        &c.speed_list,
        &c.speed_menu,
        &c.speed_readout,
    );
    let mut g = c.video_pref.borrow_mut();
    if g.smooth_60 {
        let r = resync_smooth_speed(&c.player, &mut g, listed);
        if r.smooth_auto_off {
            sync_smooth_60_to_off(&c.app);
        }
    }
}

fn resync_smooth_speed(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    vp: &mut db::VideoPrefs,
    listed: Option<f64>,
) -> video_pref::MpvVideoApply {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return video_pref::MpvVideoApply::default();
    };
    let r = if let Some(s) = listed {
        video_pref::refresh_smooth_for_playback_speed(player, vp, Some(s))
    } else if video_pref::needs_playback_speed_env_resync(&b.mpv) {
        video_pref::refresh_smooth_for_playback_speed(player, vp, None)
    } else {
        video_pref::resync_smooth_if_speed_mismatch(player, vp)
    };
    drop(g);
    r
}
