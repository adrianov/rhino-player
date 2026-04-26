# Rhino Player

A media player for Linux (GNOME, Ubuntu, and similar) that uses **mpv** for decoding and **GTK 4** with **libadwaita** for the user interface. The implementation language is **Rust**. The long-term goal is a reliable distributable, with **static** or mostly self-contained release artifacts where the platform allows; see [docs/features/20-static-build.md](docs/features/20-static-build.md).

## Current state

A working **single-window** player shell is in place: `Adw` application + `ToolbarView` (header, video area, bottom transport + seek + times). Video is **libmpv** render output into a **`GtkGLArea`** (OpenGL / EGL) on X11 and Wayland. The main content is wrapped in **`GtkWindowHandle`** so you can **drag the window from the video area** as well as the titlebar. UI chrome (header and bottom bar) can **auto-hide** after idle; the pointer can hide over the video.

**Playback and data:** play / pause, seek bar and keyboard shortcuts, **volume** and **mute** (header and scroll on the video, persisted in **SQLite**), **audio track** selection in the sound popover, **main menu → Preferences** for optional **Smooth video (60 FPS)** (VapourSynth + bundled mvtools `.vpy` or a custom path), and **libmpv** `watch-later`–style **resume** via a dedicated XDG `watch_later` directory. **Open** from the main menu (Ctrl+O) and optional **CLI** path on launch.

**When nothing is open:** a **“continue”** grid of **recent** local files (with thumbnails) is shown; entries come from **history** in the DB. **End of file** can **advance to the next** file in the same directory or a **sibling** subfolder (sibling queue). The continue grid supports **dismiss** and a short **undo** path.

**Not done yet (roadmap):** full playlist / shuffle / loop UI, MPRIS2, DnD + URL/yt-dlp, chapters, track menus beyond audio, video filters, global prefs UI, **reliable** post–manual-resize **aspect lock** (Wayland; see [17-window-behavior](docs/features/17-window-behavior.md)), and more. See the feature index: [docs/README.md](docs/README.md).

## Document-first development

We specify behavior in `docs/` before (or in step with) implementation. The index is [docs/README.md](docs/README.md). Cursor / agent rules in `.cursor/rules/document-first.mdc` enforce this for AI-assisted work.

## Desktop integration (icon, launcher, AppStream)

The app id and icon name are **`ch.rhino.RhinoPlayer`** (GNOME [application id]). Bundled assets live under `data/`:

- **`data/icons`**: Freedesktop [icon theme] `hicolor` tree + full master PNG; see `data/icons/README.md` (trim/margin notes and `gtk-update-icon-cache`).
- **`data/applications/ch.rhino.RhinoPlayer.desktop`**: launcher; `Icon=`, `StartupWMClass=`, and `Exec=` for packaging.
- **`data/metainfo/ch.rhino.RhinoPlayer.metainfo.xml`**: AppStream metadata for software centers.

At runtime, `src/icons.rs` prepends the manifest `data/icons` directory to GTK’s search path so **About** and `gtk::Window::set_default_icon_name` can resolve the icon for in-app chrome.

**Shell taskbar / alt+tab (GNOME, etc.):** the compositor uses the **installed** `ch.rhino.RhinoPlayer.desktop` `Icon=` entry, not the window hint alone. After building, run **`./data/install-to-user-dirs.sh`** (optionally with the path to your `rhino-player` binary) so `~/.local/share/applications` and `~/.local/share/icons/hicolor` are populated; see `data/README.md`. A **`glib::set_prgname`** in `main` matches the app id for startup notification / WM mapping.

[application id]: https://developer.gnome.org/documentation/tutorials/application-id.html
[icon theme]: https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html

## Build

**Requirements:** Rust 1.74+; system packages: **GTK 4** (`libgtk-4-dev`), **libadwaita** (`libadwaita-1-dev`), **libmpv** (`libmpv-dev`), `pkg-config`, `build-essential`.

```bash
cargo build --release
./target/release/rhino-player
```

**Note:** libmpv requires a C locale for numbers. The binary sets `LC_NUMERIC=C` and `setlocale(LC_NUMERIC, "C")` before starting mpv.

## Test

```bash
cargo test
```

## Code quality (complexity, not AbcSize)

There is no Rust equivalent of **RuboCop’s `Metrics/AbcSize`**. The usual substitute is [Clippy](https://doc.rust-lang.org/clippy/)—especially **`cognitive_complexity`**, with thresholds in **`clippy.toml`**. `cargo build` does **not** run Clippy; use it in CI and before merging:

```bash
cargo clippy --all-targets --all-features
# or the project alias:
cargo qcheck
```

See **`.cursor/rules/complexity-and-module-design.mdc`** for policy on when to refactor a feature that spreads across “too many” files.

## Run

Requires a running display (Wayland or X11). From the project root:

```bash
cargo run
# or
./target/debug/rhino-player
```

You can pass a file path on the **command line** or use **Open video…** (Ctrl+O). **Audio** uses `ao=pulse` (PipeWire’s Pulse layer on many setups). If the picture is black or there is no sound, check `mpv`’s `hwdec` / `ao` when preferences land.

## Copyright

Copyright © Peter Adrianov, 2026

## License

GPL-3.0-or-later (see `Cargo.toml` and `COPYRIGHT`).
