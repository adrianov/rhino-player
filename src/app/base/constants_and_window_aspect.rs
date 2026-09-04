const APP_WIN_TITLE: &str = "Rhino Player";
/// **Preferences** row for `video_smooth_60`: stores **intent**; the bundled `.vpy` runs only at ~**1.0×**.
const SMOOTH60_MENU_LABEL: &str = "Smooth Video (60 FPS)";
const SEEK_BAR_MENU_LABEL: &str = "Progress Bar Preview";
const LICENSE_NOTICE: &str = concat!(
    "Rhino Player is licensed as GPL-3.0-or-later.\n\n",
    include_str!("../../../COPYRIGHT"),
    "\n\n",
    include_str!("../../../LICENSE")
);

/// [gio::Menu] row with optional Adwaita-style symbolic icon ([ThemedIcon]),
/// mirrored to **`verb-icon`** so GTK/OS menu layers can show the same graphic.
fn menu_append_action_icon(
    menu: &gio::Menu,
    label: Option<&str>,
    detailed_action: Option<&str>,
    icon: Option<&str>,
) {
    let item = gio::MenuItem::new(label, detailed_action);
    if let Some(name) = icon {
        let themed = gio::ThemedIcon::new(name);
        item.set_icon(&themed);
        if let Some(v) = themed.serialize() {
            item.set_attribute_value("verb-icon", Some(&v));
        }
    }
    menu.append_item(&item);
}

fn title_for_open_path(path: &Path) -> String {
    crate::playback_entity::window_title_for(path)
}

/// Keeps [`gtk::ApplicationWindow::title`] and an optional GTK header-bar label aligned (macOS title
/// widget); pass `mirror` [`None`] on Linux where the shell shows the window title natively.
fn sync_app_window_title(
    win: &adw::ApplicationWindow,
    mirror: Option<&gtk::Label>,
    title: Option<&str>,
) {
    let text = title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or(APP_WIN_TITLE);
    win.set_title(Some(text));
    if let Some(l) = mirror {
        l.set_label(text);
    }
}
const IDLE_3S: Duration = Duration::from_secs(3);
/// After chrome hides, GTK often emits spurious pointer motion/enter; ignore for this long.
const LAYOUT_SQUELCH: Duration = Duration::from_millis(450);
/// Ignore repeated motion with the same coordinates (reflows can re-emit the same (x, y)).
const COORD_EPS: f64 = 1.0;
/// Base width (px) when fitting the window to a **horizontal** video; height follows aspect ratio.
const FIT_H_VIDEO_W: i32 = 960;
const FIT_H_VIDEO_MAX_H: i32 = 900;
/// Delay so mpv can populate `dwidth` / `dheight` (or `width` / `height`) after `loadfile`.
const FIT_WINDOW_DELAY_MS: u32 = 220;
const WARM_REVEAL_DELAY_MS: u64 = 160;
const SUB_SCAN_TICKS: u8 = 24;
const SUB_SCAN_MS: u64 = 250;
const WIN_INIT_W: i32 = 960;
const WIN_INIT_H: i32 = 540;

type RcPathFn = Rc<dyn Fn(&Path)>;
type RecentBackfillJob = (Rc<RecentContext>, Vec<PathBuf>);

fn same_xy(a: f64, b: f64) -> bool {
    (a - b).abs() < COORD_EPS
}

/// State for 3s auto-hide: header [gtk::MenuButton]s delay hiding while open (sound + subs + speed + main; audio tracks are inside the sound popover).
struct ChromeBarHide {
    nav: Rc<RefCell<Option<glib::SourceId>>>,
    vol: gtk::MenuButton,
    sub: gtk::MenuButton,
    speed: gtk::MenuButton,
    main: gtk::MenuButton,
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    gl: gtk::GLArea,
    bar_show: Rc<Cell<bool>>,
    recent: gtk::Box,
    bottom: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    squelch: Rc<Cell<Option<Instant>>>,
    /// True while the user is pressing the seek thumb. Auto-hide reschedules itself instead of
    /// hiding the bars so the slider does not vanish under the cursor mid-drag.
    seek_grabbed: Rc<Cell<bool>>,
    /// First mapped `shows_*_title_buttons` snapshot; restores exact CSD layout after chrome hide.
    hdr_csd_baseline: Rc<Cell<Option<(bool, bool)>>>,
}

fn show_pointer(gl: &gtk::GLArea) {
    gl.remove_css_class("rp-cursor-hidden");
    gl.set_cursor_from_name(None);
}

/// Fullscreen is paired with a programmatic `maximize()` (CSD shows restore); GTK may not restore the
/// pre-maximize size after `unfullscreen` — we save **windowed** (w, h) before that maximize and
/// re-apply in `connect_fullscreened_notify` on leave.
fn win_normal_size(win: &adw::ApplicationWindow) -> (i32, i32) {
    let w = win.width();
    let h = win.height();
    if w >= 2 && h >= 2 {
        (w, h)
    } else {
        (WIN_INIT_W, WIN_INIT_H)
    }
}

fn same_open_target(a: &Path, b: &Path) -> bool {
    crate::video_ext::paths_same_file(
        &crate::video_ext::resolve_open_media_path(a),
        &crate::video_ext::resolve_open_media_path(b),
    )
}

/// `RHINO_ASPECT_DEBUG=1` — extra aspect logs (resize-end, sync poll).
fn aspect_debug() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("RHINO_ASPECT_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

include!("window_aspect_resize.rs");
