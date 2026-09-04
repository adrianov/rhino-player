/// Main application window shell (Chrome, CSD, default size).
fn build_main_application_window(app: &adw::Application) -> adw::ApplicationWindow {
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title(APP_WIN_TITLE)
        .icon_name(APP_ID)
        .default_width(WIN_INIT_W)
        .default_height(WIN_INIT_H)
        .css_classes(["rp-win"])
        .build();
    #[cfg(target_os = "macos")]
    win.add_css_class("rp-macos");
    win
}

struct PlaybackChromeRow {
    play_pause: gtk::Button,
    sibling_nav: SiblingNavUi,
}

/// Bottom-bar play control and prev/next sibling navigation (wrapped for hit targets).
fn build_playback_chrome_row() -> PlaybackChromeRow {
    let play_pause = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play_pause.add_css_class("flat");
    play_pause.add_css_class("rpb-play");
    play_pause.set_tooltip_text(Some("Play (Space)"));
    play_pause.set_sensitive(false);

    let (btn_prev, wrap_prev) = wrapped_icon_button("go-previous-symbolic", "rpb-prev");
    let (btn_next, wrap_next) = wrapped_icon_button("go-next-symbolic", "rpb-next");
    PlaybackChromeRow {
        play_pause,
        sibling_nav: SiblingNavUi::new(&btn_prev, &btn_next, &wrap_prev, &wrap_next),
    }
}

/// Icon button wrapped in a box for a larger hit target.
fn wrapped_icon_button(icon: &'static str, css: &'static str) -> (gtk::Button, gtk::Box) {
    let btn = gtk::Button::from_icon_name(icon);
    btn.add_css_class("flat");
    btn.add_css_class(css);
    btn.set_sensitive(false);
    let wrap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    wrap.set_can_target(true);
    wrap.append(&btn);
    (btn, wrap)
}

/// libmpv render target. Linux uses GTK GL; macOS treats this as a transparent
/// sizing placeholder above the native CAOpenGLLayer (`mpv_embed::macos_video_attach`).
fn build_gl_video_area() -> gtk::GLArea {
    let gl_area = gtk::GLArea::new();
    gl_area.add_css_class("rp-gl");
    if cfg!(target_os = "macos") {
        gl_area.add_css_class("rp-gl-native");
    }
    gl_area.set_hexpand(true);
    gl_area.set_vexpand(true);
    gl_area.set_auto_render(false);
    gl_area.set_has_stencil_buffer(false);
    gl_area.set_has_depth_buffer(false);
    gl_area.set_can_focus(false);
    gl_area.set_focus_on_click(false);
    gl_area
}

include!("widgets_seek_time.rs");

include!("widgets_header_shell.rs");
