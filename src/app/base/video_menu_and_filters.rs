#[cfg_attr(target_os = "macos", allow(dead_code))]
fn video_file_filter() -> gtk::FileFilter {
    let f = gtk::FileFilter::new();
    f.set_name(Some("Video Files"));
    {
        f.add_mime_type("video/*");
        for s in video_ext::SUFFIX {
            f.add_suffix(s);
        }
        f.add_suffix("bdmv");
        f.add_suffix("bdm");
    }
    #[cfg(target_os = "macos")]
    add_macos_video_patterns(&f);
    f
}

/// macOS NSSavePanel/OpenPanel ignores some suffix rules; add explicit glob patterns per suffix.
#[cfg(target_os = "macos")]
fn add_macos_video_patterns(f: &gtk::FileFilter) {
    for s in video_ext::SUFFIX {
        f.add_pattern(&format!("*.{s}"));
        let up = s.to_uppercase();
        if up.as_str() != *s {
            f.add_pattern(&format!("*.{up}"));
        }
    }
    f.add_pattern("*.bdmv");
    f.add_pattern("*.BDMV");
    f.add_pattern("*.bdm");
    f.add_pattern("*.BDM");
}

fn vpy_file_filter() -> gtk::FileFilter {
    let f = gtk::FileFilter::new();
    f.set_name(Some("VapourSynth Scripts"));
    f.add_suffix("vpy");
    #[cfg(target_os = "macos")]
    {
        f.add_pattern("*.vpy");
        f.add_pattern("*.VPY");
    }
    f
}

include!("toolbar_reveal_set.rs");

/// Rebuilds the **Preferences** submenu: Smooth 60, seek preview, optional `basename` for `video_vs_path`
/// ([vs-custom]), [choose-vs].
fn video_pref_submenu_rebuild(m: &gio::Menu, p: &db::VideoPrefs, app: &adw::Application) {
    m.remove_all();
    menu_append_action_icon(
        m,
        Some(SMOOTH60_MENU_LABEL),
        Some("app.smooth-60"),
        Some("camera-video-symbolic"),
    );
    menu_append_action_icon(
        m,
        Some(SEEK_BAR_MENU_LABEL),
        Some("app.seek-bar-preview"),
        Some("sidebar-show-symbolic"),
    );
    append_vs_custom_submenu_row(m, p);
    menu_append_action_icon(
        m,
        Some("Choose VapourSynth Script (.vpy)…"),
        Some("app.choose-vs"),
        Some("document-properties-symbolic"),
    );
    sync_vs_custom_action_state(app, p);
}

/// Row showing the chosen script's basename when a custom VapourSynth script is configured.
fn append_vs_custom_submenu_row(m: &gio::Menu, p: &db::VideoPrefs) {
    if !p.vs_path.trim().is_empty() {
        let name = std::path::Path::new(p.vs_path.trim())
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("script.vpy");
        menu_append_action_icon(
            m,
            Some(name),
            Some("app.vs-custom"),
            Some("text-x-generic-symbolic"),
        );
    }
}

/// Mirror the current custom-script preference into the [vs-custom] action state.
fn sync_vs_custom_action_state(app: &adw::Application, p: &db::VideoPrefs) {
    if let Some(a) = app
        .lookup_action("vs-custom")
        .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
    {
        a.set_state(&(!p.vs_path.trim().is_empty()).to_variant());
    }
}

include!("video_smooth_60_toggle.rs");
include!("video_app_actions_register.rs");
