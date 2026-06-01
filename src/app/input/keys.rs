/// When focus is in a widget that needs unmodified key events (typing, caret moves), let GTK handle
/// keys after our [`gtk::PropagationPhase::Capture`] pass — except [`gtk::gdk::Key::Escape`],
/// which is handled above this check in [`w_in_key_controller`].
fn root_focus_wants_raw_keys(win: &adw::ApplicationWindow) -> bool {
    let Some(fw) = gtk::prelude::RootExt::focus(win) else {
        return false;
    };
    fw.downcast_ref::<gtk::TextView>().is_some()
        || fw.downcast_ref::<gtk::Entry>().is_some()
        || fw.downcast_ref::<gtk::SearchEntry>().is_some()
        || fw.downcast_ref::<gtk::SpinButton>().is_some()
        || fw.downcast_ref::<gtk::PasswordEntry>().is_some()
}

/// GDK **Audio\*** keys: hardware play/pause/stop and prev/next on Linux (and keyboards that expose them via GDK).
#[cfg(not(target_os = "macos"))]
fn propagation_for_media_keys(
    key: gtk::gdk::Key,
    play_key: &PlayToggleCtx,
    nav: &SiblingNavTryRefs,
) -> Option<glib::Propagation> {
    if key == gtk::gdk::Key::AudioPlay || key == gtk::gdk::Key::AudioPause {
        let _ = toggle_play_pause(play_key);
        return Some(glib::Propagation::Stop);
    }
    if key == gtk::gdk::Key::AudioStop {
        media_stop(play_key);
        return Some(glib::Propagation::Stop);
    }
    if key == gtk::gdk::Key::AudioPrev {
        try_load_sibling_pick(sibling_advance::prev_before_current, "previous", nav);
        return Some(glib::Propagation::Stop);
    }
    if key == gtk::gdk::Key::AudioNext {
        try_load_sibling_pick(sibling_advance::next_after_eof, "next", nav);
        return Some(glib::Propagation::Stop);
    }
    None
}

/// macOS hardware keys use [`wire_macos_now_playing_remote`] only (`MPRemoteCommandCenter`).
#[cfg(target_os = "macos")]
#[inline]
fn propagation_for_media_keys(
    _key: gtk::gdk::Key,
    _play_key: &PlayToggleCtx,
    _nav: &SiblingNavTryRefs,
) -> Option<glib::Propagation> {
    None
}

fn w_in_key_controller(ctx: &WindowInputCtx) {
    let p = ctx.player.clone();
    let win_key = ctx.shell.win.clone();
    let recent_esc = ctx.shell.recent.clone();
    let browse_back = ctx.on_browse_back.clone();
    let fr_key = ctx.fs_restore.clone();
    let lu_key = ctx.last_unmax.clone();
    let skip_key = ctx.skip_max_to_fs.clone();
    let fs_esc_busy = Rc::clone(&ctx.fs_transition_busy);
    let play_key = PlayToggleCtx {
        app: ctx.app.clone(),
        player: p.clone(),
        video_pref: Rc::clone(&ctx.video_pref),
        win: win_key.clone(),
        video_handle: ctx.shell.video_handle.clone(),
        gl: ctx.shell.gl.clone(),
        recent: recent_esc.clone(),
        last_path: ctx.last_path.clone(),
        on_video_chrome: ctx.on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&ctx.on_file_loaded),
        win_aspect: ctx.win_aspect.clone(),
        sub_menu: None,
        play_pause: ctx.play_pause.clone(),
        hdr_title_mirror: ctx.hdr_title_mirror.clone(),
        playback_focus: Rc::clone(&ctx.playback_focus),
    };
    let seek_sc = ctx.seek.clone();
    let seek_sync_sc = ctx.seek_sync.clone();
    let time_left_sc = ctx.time_left.clone();
    let gl_seek = ctx.shell.gl.clone();
    let smooth_seek_db = ctx.smooth_seek_debounce.clone();
    let dvd_bar_keys = Rc::clone(&ctx.dvd_bar);
    let resume_seek_idle = ctx.resume_after_seek_idle.clone();
    let play_seek_toggle = ctx.play_toggle.clone();
    let digit_spd = DigitSpeedShortcutCtx {
        player: p.clone(),
        play_toggle: ctx.play_toggle.clone(),
        gl: ctx.shell.gl.clone(),
        video_pref: Rc::clone(&ctx.video_pref),
        app: ctx.app.clone(),
        speed_sync: ctx.speed_sync.clone(),
        speed_menu: ctx.speed_menu.clone(),
        speed_list: ctx.speed_list.clone(),
        speed_readout: ctx.speed_readout.clone(),
    };
    let last_path_nav = ctx.last_path.clone();
    let on_vid_nav = ctx.on_video_chrome.clone();
    let win_aspect_nav = ctx.win_aspect.clone();
    let seof_nav = ctx.sibling_seof.clone();
    let on_loaded_nav = ctx.on_file_loaded.clone();
    let hdr_mirror_nav = ctx.hdr_title_mirror.clone();
    let playback_nav = Rc::clone(&ctx.playback_focus);
    let video_pref_nav = ctx.video_pref.clone();
    let k = gtk::EventControllerKey::new();
    // Capture phase: run before the focused widget (e.g. bottom-bar buttons, scales) so Space /
    // Enter / arrows trigger playback shortcuts instead of GTK's button activation / focus
    // navigation defaults.
    k.set_propagation_phase(gtk::PropagationPhase::Capture);
    k.connect_key_pressed(move |_c, key, _code, m| {
        if let Some(r) = propagation_escape_key(key, &recent_esc, &p, &browse_back) {
            return r;
        }
        let nav = SiblingNavTryRefs {
            player: p.clone(),
            win: win_key.clone(),
            gl: gl_seek.clone(),
            recent: recent_esc.clone(),
            last_path: last_path_nav.clone(),
            video_pref: video_pref_nav.clone(),
            on_video_chrome: on_vid_nav.clone(),
            win_aspect: win_aspect_nav.clone(),
            sibling_seof: seof_nav.clone(),
            on_file_loaded: on_loaded_nav.clone(),
            hdr_title_mirror: hdr_mirror_nav.clone(),
            playback_focus: playback_nav.clone(),
        };
        if let Some(r) = propagation_for_media_keys(key, &play_key, &nav) {
            return r;
        }
        if root_focus_wants_raw_keys(&win_key) {
            return glib::Propagation::Proceed;
        }
        if (key == gtk::gdk::Key::c || key == gtk::gdk::Key::C) && copy_path_modifier_held(m) {
            if try_copy_playing_path(&p) {
                return glib::Propagation::Stop;
            }
            return glib::Propagation::Proceed;
        }
        if key == gtk::gdk::Key::Return
            || key == gtk::gdk::Key::KP_Enter
            || key == gtk::gdk::Key::f
            || key == gtk::gdk::Key::F
        {
            toggle_fullscreen(
                &win_key,
                &fr_key,
                &lu_key,
                &skip_key,
                fs_esc_busy.as_ref(),
            );
            return glib::Propagation::Stop;
        }
        if key == gtk::gdk::Key::m || key == gtk::gdk::Key::M {
            let g = p.borrow();
            let Some(b) = g.as_ref() else {
                return glib::Propagation::Proceed;
            };
            let muted = b.mpv.get_property::<bool>("mute").unwrap_or(false);
            if b.mpv.set_property("mute", !muted).is_err() {
                return glib::Propagation::Proceed;
            }
            return glib::Propagation::Stop;
        }
        if let Some(r) = try_digit_speed_shortcut(key, m, &digit_spd) {
            return r;
        }
        if key == gtk::gdk::Key::Up {
            let g = p.borrow();
            let Some(b) = g.as_ref() else {
                return glib::Propagation::Proceed;
            };
            nudge_mpv_volume(&b.mpv, 5.0);
            return glib::Propagation::Stop;
        }
        if key == gtk::gdk::Key::Down {
            let g = p.borrow();
            let Some(b) = g.as_ref() else {
                return glib::Propagation::Proceed;
            };
            nudge_mpv_volume(&b.mpv, -5.0);
            return glib::Propagation::Stop;
        }
        if m.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            if key == gtk::gdk::Key::Left || key == gtk::gdk::Key::KP_Left {
                try_load_sibling_pick(sibling_advance::prev_before_current, "previous", &nav);
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Right || key == gtk::gdk::Key::KP_Right {
                try_load_sibling_pick(sibling_advance::next_after_eof, "next", &nav);
                return glib::Propagation::Stop;
            }
        }
        let seek_deps = SeekArrowDeps {
            player: &p,
            seek: &seek_sc,
            seek_sync: &seek_sync_sc,
            time_left: &time_left_sc,
            gl: &gl_seek,
            smooth_seek_debounce: &smooth_seek_db,
            resume_after_seek_idle: &resume_seek_idle,
            play_toggle: &play_seek_toggle,
            dvd_bar: Some(&dvd_bar_keys),
        };
        if let Some(r) =
            propagation_horizontal_seek(key, recent_esc.is_visible(), seek_sc.is_sensitive(), &seek_deps)
        {
            return r;
        }
        if key != gtk::gdk::Key::space {
            return glib::Propagation::Proceed;
        }
        if !toggle_play_pause(&play_key) {
            return glib::Propagation::Proceed;
        }
        glib::Propagation::Stop
    });
    ctx.shell.win.add_controller(k);
}
