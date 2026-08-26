/// Final-action refs, group A (indexed tuples keep the assembly under AbcSize budgets).
type WapFinalRefsA = (
    adw::Application,
    adw::ApplicationWindow,
    Rc<RefCell<Option<(i32, i32)>>>,
    Rc<Cell<bool>>,
    Rc<RefCell<(i32, i32)>>,
    Rc<Cell<bool>>,
    adw::ToolbarView,
    adw::HeaderBar,
    gtk::GLArea,
    gtk::Box,
    gtk::Box,
);

fn wap_final_refs_a(args: &WindowAfterPresentArgs) -> WapFinalRefsA {
    (
        args.app.clone(),
        args.w.win.clone(),
        args.fs_restore.clone(),
        args.fs_transition_busy.clone(),
        args.last_unmax.clone(),
        args.skip_max_to_fs.clone(),
        args.w.root.clone(),
        args.w.header.clone(),
        args.w.gl_area.clone(),
        args.w.recent_scrl.clone(),
        args.w.bottom.clone(),
    )
}

/// Final-action refs, group B.
type WapFinalRefsB = (
    Rc<RefCell<Option<MpvBundle>>>,
    Rc<RefCell<db::SubPrefs>>,
    Rc<RefCell<db::VideoPrefs>>,
    Rc<Cell<bool>>,
    gio::Menu,
    Rc<Cell<bool>>,
    Rc<RefCell<Option<PathBuf>>>,
    Rc<dyn Fn()>,
    Rc<dyn Fn()>,
    Rc<WinAspectCell>,
    Rc<Cell<bool>>,
);

fn wap_final_refs_b(args: &WindowAfterPresentArgs) -> WapFinalRefsB {
    (
        args.player.clone(),
        args.sub_pref.clone(),
        args.video_pref.clone(),
        args.playback_focus.clone(),
        args.w.pref_menu.clone(),
        args.seek_bar_on.clone(),
        args.last_path.clone(),
        args.on_video_chrome.clone(),
        args.on_file_loaded.clone(),
        args.win_aspect.clone(),
        args.bar_show.clone(),
    )
}

/// Final-action refs, group C.
type WapFinalRefsC = (
    Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    Rc<Cell<bool>>,
    Rc<Cell<bool>>,
    Rc<Cell<Option<(bool, bool)>>>,
    Option<Rc<gtk::Label>>,
    gtk::Button,
    gtk::Label,
    Rc<dyn Fn(String)>,
);

fn wap_final_refs_c(args: &WindowAfterPresentArgs) -> WapFinalRefsC {
    (
        args.idle_inhib.clone(),
        args.exit_after_current.clone(),
        args.mpv_teardown_after_draw.clone(),
        args.hdr_csd_baseline.clone(),
        args.hdr_title_mirror.clone(),
        args.w.smooth_btn.clone(),
        args.w.smooth_status.clone(),
        args.on_open_fail.clone(),
    )
}

/// Final actions (open dialog, quit/close, fullscreen toggle, accels) wiring step.
fn wire_final_actions_step(args: &WindowAfterPresentArgs) {
    let fa = wap_final_refs_a(args);
    let fb = wap_final_refs_b(args);
    let fc = wap_final_refs_c(args);
    wire_final_actions(FinalActionCtx {
        app: fa.0,
        win: fa.1,
        fs_restore: fa.2,
        fs_transition_busy: fa.3,
        last_unmax: fa.4,
        skip_max_to_fs: fa.5,
        root: fa.6,
        header: fa.7,
        gl: fa.8,
        recent: fa.9,
        bottom: fa.10,
        player: fb.0,
        sub_pref: fb.1,
        video_pref: fb.2,
        playback_focus: fb.3,
        #[cfg(target_os = "macos")]
        main_menu: args.w.main_menu.clone(),
        pref_menu: fb.4,
        seek_bar_on: fb.5,
        last_path: fb.6,
        on_video_chrome: fb.7,
        on_file_loaded: fb.8,
        win_aspect: fb.9,
        bar_show: fb.10,
        idle_inhib: fc.0,
        exit_after_current: fc.1,
        mpv_teardown_after_draw: fc.2,
        hdr_csd_baseline: fc.3,
        hdr_title_mirror: fc.4,
        smooth_toolbar_btn: fc.5,
        smooth_toolbar_status: fc.6,
        on_open_fail: fc.7,
    });
}
