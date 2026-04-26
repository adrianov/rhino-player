# Rhino Player

A media player for Linux (GNOME, Ubuntu, and similar) that uses **mpv** for decoding and **GTK 4** with **libadwaita** for the user interface. The implementation language is **Rust**. The long-term goal is a reliable distributable, with **static** or mostly self-contained release artifacts where the platform allows; see [docs/features/20-static-build.md](docs/features/20-static-build.md).

## Current state

A working **single-window** player shell is in place: `Adw` application + `ToolbarView` (header, video area, bottom transport + seek + times). Video is **libmpv** render output into a **`GtkGLArea`** (OpenGL / EGL) on X11 and Wayland. The main content is wrapped in **`GtkWindowHandle`** so you can **drag the window from the video area** as well as the titlebar. UI chrome (header and bottom bar) can **auto-hide** after idle; the pointer can hide over the video.

**Playback and data:** play / pause, seek bar and keyboard shortcuts, optional **hover preview** (thumbnail of the frame under the pointer on the progress bar; [18-thumbnail-preview](docs/features/18-thumbnail-preview.md)), **volume** and **mute** (header and scroll on the video, persisted in **SQLite**), **audio track** selection in the sound popover, **main menu → Preferences** for optional **Smooth video (~60 FPS at 1.0×)** (VapourSynth + bundled mvtools `.vpy` or a custom path; effect at **~1.0×** only), and **libmpv** `watch-later`–style **resume** via a dedicated XDG `watch_later` directory. **Open** from the main menu (Ctrl+O) and optional **CLI** path on launch.

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
[vapoursynth-mvtools]: https://github.com/dubhater/vapoursynth-mvtools

## Build

**Requirements:** Rust 1.74+; system packages: **GTK 4** (`libgtk-4-dev`), **libadwaita** (`libadwaita-1-dev`), **libmpv** (`libmpv-dev`), `pkg-config`, `build-essential`.

```bash
cargo build --release
./target/release/rhino-player
```

**Note:** libmpv requires a C locale for numbers. The binary sets `LC_NUMERIC=C` and `setlocale(LC_NUMERIC, "C")` before starting mpv.

## Smooth 60 FPS on Ubuntu

Rhino’s **Preferences → Smooth video (~60 FPS at 1.0×)** uses mpv’s native **VapourSynth** video filter plus **MVTools** (`libmvtools.so`). Ubuntu’s default `mpv` / `libmpv` packages often do **not** include the `vapoursynth` filter, so installing only Rhino is not enough.

Check the current system first:

```bash
mpv -vf help 2>&1 | grep -E '^[[:space:]]*vapoursynth[[:space:]]'
python3 -c 'import vapoursynth as vs; print(vs.core.mv)'
```

The first command must print a `vapoursynth` filter. The second command must print an MVTools object instead of failing. If either check fails, use the project helper:

```bash
./scripts/ensure-vapoursynth-debian.sh
```

If the helper says Ubuntu’s stock or PPA mpv is missing VapourSynth support, build mpv + libmpv with VapourSynth enabled:

```bash
./scripts/build-mpv-vapoursynth-system.sh
```

That script builds mpv/libmpv to `/usr/local` with `-Dvapoursynth=enabled`. Afterward, make sure Rhino loads that libmpv, for example:

```bash
export LD_LIBRARY_PATH=/usr/local/lib/x86_64-linux-gnu:/usr/local/lib:$LD_LIBRARY_PATH
cargo run
```

Install MVTools through the distro package when available:

```bash
sudo apt-get install vapoursynth vapoursynth-python3 vapoursynth-mvtools
```

If `vapoursynth-mvtools` is unavailable, install MVTools with `vsrepo` or build [vapoursynth-mvtools] from source. Rhino looks for `libmvtools.so` in common system locations, pipx/vsrepo under `~/.local`, and a bounded `~/.local` search. You can also force the path:

```bash
export RHINO_MVTOOLS_LIB=/full/path/to/libmvtools.so
```

Once the checks pass, start Rhino, open a video, and enable **Preferences → Smooth video (~60 FPS at 1.0×)**. The built-in `data/vs/rhino_60_mvtools.vpy` script is used by default; **Choose VapourSynth script…** is only needed for a custom `.vpy`.

Notes:

- Smooth 60 runs only at about **1.0×** playback speed. At 1.5× / 2.0× Rhino intentionally skips the filter.
- While the VapourSynth filter is active, Rhino forces software decode (`hwdec=no`) so frames pass through the CPU filter path. Expect higher CPU use; 1080p may need a strong CPU.
- A brief black frame while the graph starts is normal. Judge motion on camera pans; subtitles are drawn after the video filter and are not a reliable test.
- If mpv rejects the filter, Rhino turns the preference off and saves that state. Fix mpv/VapourSynth/MVTools, then enable the menu item again.

More detail: [docs/features/26-sixty-fps-motion.md](docs/features/26-sixty-fps-motion.md) and [data/vs/README.md](data/vs/README.md).

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
