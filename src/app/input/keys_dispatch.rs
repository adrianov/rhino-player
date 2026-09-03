// Snapshot of cloned input-context handles taken once when the capture-phase key controller is
// wired (`w_in_key_controller`); every key press routes through `KeyDispatch::dispatch` into the
// per-family handlers (`keys_families.rs`) in fixed precedence order. Grouped refs live in
// `keys_handles.rs`.
struct KeyDispatch {
    p: Rc<RefCell<Option<MpvBundle>>>,
    win_key: adw::ApplicationWindow,
    recent_esc: gtk::Box,
    browse_back: Rc<dyn Fn(bool)>,
    app: adw::Application,
    fs: FullscreenKeyRefs,
    play_key: PlayToggleCtx,
    seek_keys: SeekArrowKeys,
    digit_spd: DigitSpeedShortcutCtx,
    nav: NavHandleSnapshot,
}

impl KeyDispatch {
    fn new(ctx: &WindowInputCtx) -> Self {
        let p = ctx.player.clone();
        let win_key = ctx.shell.win.clone();
        let recent_esc = ctx.shell.recent.clone();
        Self {
            browse_back: ctx.on_browse_back.clone(),
            app: ctx.app.clone(),
            fs: FullscreenKeyRefs::new(ctx),
            seek_keys: SeekArrowKeys::new(ctx),
            nav: NavHandleSnapshot::new(ctx),
            play_key: play_toggle_ctx_for_keys(ctx, &p, &win_key, &recent_esc),
            digit_spd: digit_speed_ctx_for_keys(ctx, &p),
            p,
            win_key,
            recent_esc,
        }
    }

    fn nav_refs(&self) -> SiblingNavTryRefs {
        SiblingNavTryRefs {
            player: self.p.clone(),
            win: self.win_key.clone(),
            gl: self.seek_keys.gl.clone(),
            recent: self.recent_esc.clone(),
            last_path: self.nav.last_path.clone(),
            video_pref: self.nav.video_pref.clone(),
            on_video_chrome: self.nav.on_video_chrome.clone(),
            win_aspect: self.nav.win_aspect.clone(),
            sibling_seof: self.nav.sibling_seof.clone(),
            on_file_loaded: self.nav.on_file_loaded.clone(),
            hdr_title_mirror: self.nav.hdr_title_mirror.clone(),
            playback_focus: self.nav.playback_focus.clone(),
            on_open_fail: self.nav.on_open_fail.clone(),
        }
    }

    fn dispatch(&self, key: gtk::gdk::Key, m: gtk::gdk::ModifierType) -> glib::Propagation {
        if key == gtk::gdk::Key::Escape && root_focus_wants_raw_keys(&self.win_key) {
            // A text widget owns focus (e.g. the continue-screen neighbour search box): it
            // consumes Escape itself — clear text, not strip-escape / fullscreen exits.
            return glib::Propagation::Proceed;
        }
        if let Some(r) =
            propagation_escape_key(key, &self.recent_esc, &self.p, &self.browse_back, &self.app)
        {
            return r;
        }
        let nav = self.nav_refs();
        if let Some(r) = propagation_for_media_keys(key, &self.play_key, &nav) {
            return r;
        }
        if root_focus_wants_raw_keys(&self.win_key) {
            return glib::Propagation::Proceed;
        }
        self.dispatch_shortcuts(key, m, &nav)
    }

    fn dispatch_shortcuts(
        &self,
        key: gtk::gdk::Key,
        m: gtk::gdk::ModifierType,
        nav: &SiblingNavTryRefs,
    ) -> glib::Propagation {
        if let Some(r) = quit_key(key, m, &self.app) {
            return r;
        }
        if let Some(r) = copy_playing_path_key(key, m, &self.p) {
            return r;
        }
        if let Some(r) = fullscreen_entry_key(
            key,
            &self.win_key,
            &self.fs.fr,
            &self.fs.lu,
            &self.fs.skip,
            &self.fs.fs_busy,
        ) {
            return r;
        }
        if let Some(r) = mute_toggle_key(key, &self.p) {
            return r;
        }
        if let Some(r) = try_digit_speed_shortcut(key, m, &self.digit_spd) {
            return r;
        }
        if let Some(r) = volume_nudge_key(key, &self.p) {
            return r;
        }
        if let Some(r) = ctrl_arrow_sibling_key(key, m, nav) {
            return r;
        }
        if let Some(r) = self.horizontal_seek_outcome(key) {
            return r;
        }
        space_play_key(key, &self.play_key)
    }

    /// Left / Right arrows drive the seek bar when the grid is hidden and the bar is enabled.
    fn horizontal_seek_outcome(&self, key: gtk::gdk::Key) -> Option<glib::Propagation> {
        let seek_deps = SeekArrowDeps {
            player: &self.p,
            seek: &self.seek_keys.seek,
            seek_sync: &self.seek_keys.seek_sync,
            time_left: &self.seek_keys.time_left,
            gl: &self.seek_keys.gl,
            smooth_seek_debounce: &self.seek_keys.smooth_seek_debounce,
            resume_after_seek_idle: &self.seek_keys.resume_after_seek_idle,
            play_toggle: &self.seek_keys.play_toggle,
            dvd_bar: Some(&self.seek_keys.dvd_bar),
        };
        propagation_horizontal_seek(
            key,
            self.recent_esc.is_visible(),
            self.seek_keys.seek.is_sensitive(),
            &seek_deps,
        )
    }
}
