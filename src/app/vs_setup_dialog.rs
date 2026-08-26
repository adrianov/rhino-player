#[cfg(target_os = "macos")]
const SMOOTH_SETUP_TEXT: &str = SMOOTH_SETUP_TEXT_MACOS;
#[cfg(not(target_os = "macos"))]
const SMOOTH_SETUP_TEXT: &str = SMOOTH_SETUP_TEXT_LINUX;

#[cfg(target_os = "macos")]
const SMOOTH_SETUP_HEADLINE: &str =
    "Rhino could not enable Smooth 60 FPS. Run `brew install mpv vapoursynth vapoursynth-mvtools` \
     (MVTools is then vendored under ~/.config/rhino/lib), then enable Smooth Video again.";
#[cfg(not(target_os = "macos"))]
const SMOOTH_SETUP_HEADLINE: &str =
    "Rhino could not enable Smooth 60 FPS. Install VapourSynth, MVTools, and an mpv/libmpv build \
     with VapourSynth support, then enable Smooth Video again.";

fn can_find_mvtools(v: &db::VideoPrefs) -> bool {
    if crate::paths::mvtools_from_env().is_some() {
        return true;
    }
    can_find_mvtools_os(v)
}

#[cfg(target_os = "macos")]
fn can_find_mvtools_os(_v: &db::VideoPrefs) -> bool {
    // Ignore sticky Homebrew Cellar paths in settings; do not seed as a side effect of the check.
    crate::paths::macos_mvtools_available()
}

#[cfg(not(target_os = "macos"))]
fn can_find_mvtools_os(v: &db::VideoPrefs) -> bool {
    let cached = std::path::Path::new(v.mvtools_lib.trim());
    cached.is_file() || crate::paths::mvtools_lib_search().is_some()
}

include!("vs_setup_dialog/setup_text.rs");

/// Copy-paste setup instructions shown when Smooth 60 cannot attach its VapourSynth filter.
fn show_smooth_setup_dialog(app: &adw::Application) {
    let win = make_setup_window(app);
    win.set_child(Some(&setup_dialog_content(&win)));
    win.present();
}

fn make_setup_window(app: &adw::Application) -> gtk::Window {
    let parent = app.active_window();
    let win = gtk::Window::builder()
        .modal(true)
        .title("Set Up Smooth 60 FPS")
        .default_width(720)
        .default_height(520)
        .build();
    if let Some(parent) = parent.as_ref() {
        win.set_transient_for(Some(parent));
    }
    win.set_application(Some(app));
    win
}

fn setup_dialog_content(win: &gtk::Window) -> gtk::Box {
    let area = setup_dialog_container();
    area.append(&setup_headline_label());
    area.append(&setup_scroll());
    area.append(&setup_close_button(win));
    area
}

fn setup_dialog_container() -> gtk::Box {
    let area = gtk::Box::new(gtk::Orientation::Vertical, 12);
    area.set_spacing(12);
    area.set_margin_top(16);
    area.set_margin_bottom(16);
    area.set_margin_start(16);
    area.set_margin_end(16);
    area
}

fn setup_headline_label() -> gtk::Label {
    let msg = gtk::Label::new(Some(SMOOTH_SETUP_HEADLINE));
    msg.set_wrap(true);
    msg.set_xalign(0.0);
    msg
}

fn setup_scroll() -> gtk::ScrolledWindow {
    let text = setup_dialog_text_view();
    gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&text)
        .build()
}

fn setup_close_button(win: &gtk::Window) -> gtk::Button {
    let close = gtk::Button::with_label("Close");
    close.set_halign(gtk::Align::End);
    close.connect_clicked({
        let win = win.clone();
        move |_| win.close()
    });
    close
}

fn setup_dialog_text_view() -> gtk::TextView {
    let text = gtk::TextView::new();
    text.set_editable(false);
    text.set_cursor_visible(false);
    text.set_monospace(true);
    text.buffer().set_text(SMOOTH_SETUP_TEXT);
    text
}
