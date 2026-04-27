const SMOOTH_SETUP_TEXT: &str = r#"# Debian/Ubuntu package names:
sudo apt-get install vapoursynth vapoursynth-python3 pipx p7zip-full
pipx install vsrepo
pipx ensurepath

# Open a new terminal after pipx ensurepath, then:
vsrepo update
vsrepo install mvtools

# Verify:
mpv -vf help 2>&1 | grep -E '^[[:space:]]*vapoursynth[[:space:]]'
python3 - <<'PY'
from pathlib import Path
import vapoursynth as vs

try:
    print(vs.core.mv)
except AttributeError:
    hits = sorted(Path.home().glob(
        ".local/share/pipx/venvs/vsrepo/lib/python*/site-packages/"
        "vapoursynth/plugins/vsrepo/libmvtools.so"
    ))
    if not hits:
        raise
    vs.core.std.LoadPlugin(str(hits[0]))
    print(vs.core.mv)
PY

# If mpv has no vapoursynth filter:
./scripts/build-mpv-vapoursynth-system.sh"#;

fn can_find_mvtools(v: &db::VideoPrefs) -> bool {
    let cached = std::path::Path::new(v.mvtools_lib.trim());
    cached.is_file()
        || crate::paths::mvtools_from_env().is_some()
        || crate::paths::mvtools_lib_search().is_some()
}

/// Copy-paste setup instructions shown when Smooth 60 cannot find `libmvtools.so`.
fn show_smooth_setup_dialog(app: &adw::Application) {
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

    let area = gtk::Box::new(gtk::Orientation::Vertical, 12);
    area.set_spacing(12);
    area.set_margin_top(16);
    area.set_margin_bottom(16);
    area.set_margin_start(16);
    area.set_margin_end(16);

    let msg = gtk::Label::new(Some(
        "Rhino could not find libmvtools.so for Smooth 60 FPS. Copy and run these commands, then enable Smooth Video again.",
    ));
    msg.set_wrap(true);
    msg.set_xalign(0.0);
    area.append(&msg);

    let text = gtk::TextView::new();
    text.set_editable(false);
    text.set_cursor_visible(false);
    text.set_monospace(true);
    text.buffer().set_text(SMOOTH_SETUP_TEXT);

    let scroll = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&text)
        .build();
    area.append(&scroll);

    let close = gtk::Button::with_label("Close");
    close.set_halign(gtk::Align::End);
    close.connect_clicked({
        let win = win.clone();
        move |_| win.close()
    });
    area.append(&close);

    win.set_child(Some(&area));
    win.present();
}
