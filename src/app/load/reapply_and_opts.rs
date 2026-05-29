#[derive(Clone)]
struct VideoReapply60 {
    vp: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
}

/// Options for [try_load] (keeps the arity clippy limit without `allow`).
struct LoadOpts {
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    record: bool,
    play_on_start: bool,
    /// Filled on success so [maybe_advance_sibling_on_eof] can resolve a path if mpv clears it at idle EOF.
    last_path: Rc<RefCell<Option<PathBuf>>>,
    /// Reveal chrome and (re)start 3s auto-hide; `None` for tests or callers without UI bundle.
    on_start: Option<Rc<dyn Fn()>>,
    /// Coded video size for aspect snap; cleared with no video.
    win_aspect: Rc<WinAspectCell>,
    /// Fuzzy subtitle auto-pick + hook after a successful `loadfile`.
    on_loaded: Option<Rc<dyn Fn()>>,
    /// Before `loadfile`, set mpv speed to **1.0** if it was changed (sibling EOF advance).
    reset_speed_to_normal: bool,
    /// macOS: mirrors the window title in [`adw::HeaderBar::title_widget`]; [`None`] on Linux CSD paths.
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    /// When set, set **true** in [reveal_ui_after_load] / delayed warm reveal — **false** from [back_to_browse].
    playback_focus: Option<Rc<Cell<bool>>>,
    /// Continue-grid hover/first-card preload: skip outgoing SQLite snapshot; see [MpvBundle::load_file_path].
    warm_preload: bool,
}

/// Bundles **`replace_media`** inputs (keeps Clippy arity down).
struct ReplaceMediaBundled {
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_start: Option<Rc<dyn Fn()>>,
    win_aspect: Rc<WinAspectCell>,
    on_loaded: Option<Rc<dyn Fn()>>,
    play_on_start: bool,
    reset_speed_to_normal: bool,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
}

impl LoadOpts {
    /// One builder for Opens (menu, drag-drop, **`open`/`activate`**, Nautilus, folder buttons, sibling EOF).
    /// Keeps **`record`: true** (recent history row); callers set **`play_on_start`**, **`reset_speed_to_normal`**, and the shared **`Rc`** fields.
    fn replace_media(b: ReplaceMediaBundled) -> Self {
        Self {
            video_pref: b.video_pref,
            record: true,
            play_on_start: b.play_on_start,
            last_path: b.last_path,
            on_start: b.on_start,
            win_aspect: b.win_aspect,
            on_loaded: b.on_loaded,
            reset_speed_to_normal: b.reset_speed_to_normal,
            hdr_title_mirror: b.hdr_title_mirror,
            playback_focus: None,
            warm_preload: false,
        }
    }
}
