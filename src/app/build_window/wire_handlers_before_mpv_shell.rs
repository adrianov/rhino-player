// Window-shell pre-MPV wiring: header menu cluster, fullscreen toggles, continue-strip visibility.

/// Header menu cluster (per-platform), popover-show wiring, and blackout hooks.
fn wire_header_cluster(
    w: &WindowWidgets,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
) {
    #[cfg(target_os = "macos")]
    wire_macos_header_menu_cluster(
        &w.root,
        &w.header,
        &w.outer_ovl,
        &w.win,
        &[
            (
                w.speed_mbtn.clone(),
                w.speed_mbtn.popover().expect("speed popover"),
                "speed",
            ),
            (w.sub_menu.clone(), w.sub_pop.clone(), "subtitles"),
            (w.vol_menu.clone(), w.vol_pop.clone(), "audio"),
        ],
    );
    #[cfg(not(target_os = "macos"))]
    header_menubtns_switch(&[
        w.speed_mbtn.clone(),
        w.sub_menu.clone(),
        w.vol_menu.clone(),
        w.menu_btn.clone(),
    ]);

    wire_popover_shows(player, w, sub_pref);
    crate::screen_blackout::wire_blackout_hooks(&w.blackout_sync);
}

/// Fullscreen toggle entry points (gesture, header button, recent-strip spacers).
fn wire_fullscreen_toggles(r: &PreMpvPhaseRefs<'_>) {
    let fs_toggle = FullscreenToggleRefs {
        fs_restore: Rc::clone(r.fs_restore),
        last_unmax: Rc::clone(r.last_unmax),
        skip_max_to_fs: Rc::clone(r.skip_max_to_fs),
        fs_transition_busy: Rc::clone(r.fs_transition_busy),
    };
    wire_gl_double_click_fullscreen(&r.w.gl_area, &r.w.win, &fs_toggle, &r.w.recent_scrl);
    wire_header_fullscreen_toggle(&r.w.header, &r.w.win, &fs_toggle, &r.w.recent_scrl);
    wire_recent_spacer_fullscreen(
        r.w.recent_spacers.clone(),
        &r.w.win,
        &fs_toggle,
        &r.w.recent_scrl,
    );
}

/// Shows the continue strip when no file is queued for boot; reports whether it is shown.
fn set_recent_strip_visible(w: &WindowWidgets, file_boot: &Rc<RefCell<Option<PathBuf>>>) -> bool {
    let want_recent = file_boot.borrow().is_none();
    w.recent_scrl.set_visible(want_recent);
    want_recent
}
