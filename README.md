# Rhino Player

<p align="center">
  <img src="data/icons/source/ch.rhino.RhinoPlayer-master-1024.png" alt="Rhino Player icon" width="240">
</p>

Rhino Player is a Linux desktop video player for GNOME, Ubuntu, and similar systems. It combines mpv playback with a GTK 4 / libadwaita interface, focused on smooth watching, quick resume, and simple local-file workflows.

## Features

- **Optional Smooth video (~60 FPS at 1.0×):** a VapourSynth + MVTools mode can synthesize smoother motion for compatible local videos.
- **Continue where you left off:** launch to a recent-video grid with thumbnails, progress, and one-click resume.
- **Folder and sibling playback:** at the end of a local file, Rhino can continue to the next video in the folder or the next sibling folder.
- **Clean playback view:** auto-hiding header and transport controls keep the video area focused.
- **Seek preview:** hover over the progress bar to preview frames before jumping.
- **Simple controls:** play/pause, seek, elapsed/remaining time, keyboard shortcuts, fullscreen, volume, mute, and scroll-wheel volume.
- **Audio track picker:** choose audio tracks from the sound popover when available.
- **Playback speed:** quick 1.0×, 1.5×, and 2.0× speed choices in the header.
- **Continue-list cleanup:** remove items from the continue grid or move local files to Trash, with session undo.
- **Desktop integration:** ships Freedesktop desktop, icon, and AppStream metadata for GNOME-style launchers and app grids.

See the full feature index in [docs/README.md](docs/README.md).

## Run

Requires a running Wayland or X11 desktop session.

```bash
cargo run
# or
./target/debug/rhino-player
```

Open a file from the main menu, with `Ctrl+O`, or by passing a path:

```bash
cargo run -- /path/to/video.mkv
```

## Install Desktop Files

For local development builds, install the launcher and icons under `~/.local/share`:

```bash
./data/install-to-user-dirs.sh
```

## Build

Requirements:

- Rust 1.74+
- GTK 4 development files (`libgtk-4-dev`)
- libadwaita development files (`libadwaita-1-dev`)
- libmpv development files (`libmpv-dev`)
- `pkg-config`
- `build-essential`

```bash
cargo build --release
./target/release/rhino-player
```

## Smooth 60 FPS Setup

Rhino’s **Preferences → Smooth video (~60 FPS at 1.0×)** uses mpv’s VapourSynth video filter plus MVTools. This is optional; normal playback works without it.

### Requirements

- `mpv` / `libmpv` built with the `vapoursynth` video filter enabled.
- VapourSynth runtime and Python bindings.
- MVTools for VapourSynth (`libmvtools.so`).
- A CPU fast enough for the target resolution. Smooth 60 is CPU-heavy.

### Install Packages

Package names vary by distribution. On Debian / Ubuntu-like systems, start with:

```bash
sudo apt-get install vapoursynth vapoursynth-python3 vapoursynth-mvtools
```

You also need an `mpv` / `libmpv` package that includes VapourSynth support. Some distro builds omit it. On Arch-like systems this is commonly available through the main packages or AUR variants; on Fedora / openSUSE it depends on the repository set and build options.

If `vapoursynth-mvtools` is unavailable, install MVTools with `vsrepo` or build [vapoursynth-mvtools] from source.

### Verify Support

```bash
mpv -vf help 2>&1 | grep -E '^[[:space:]]*vapoursynth[[:space:]]'
python3 -c 'import vapoursynth as vs; print(vs.core.mv)'
```

The first command must print a `vapoursynth` filter. The second command must print an MVTools object instead of failing.

### If mpv Is Missing VapourSynth

Install a distro package that enables VapourSynth, or build mpv/libmpv yourself with VapourSynth enabled. For mpv’s Meson build, the important option is:

```bash
-Dvapoursynth=enabled
```

If you install the custom libmpv under `/usr/local`, make sure Rhino loads it before the distro libmpv:

```bash
export LD_LIBRARY_PATH=/usr/local/lib/x86_64-linux-gnu:/usr/local/lib:$LD_LIBRARY_PATH
cargo run
```

### MVTools Path

Rhino searches common system paths, `pipx` / `vsrepo` locations under `~/.local`, and a bounded `~/.local` scan. You can also force the library path:

```bash
export RHINO_MVTOOLS_LIB=/full/path/to/libmvtools.so
```

### Helper Scripts

The project includes Debian / Ubuntu helper scripts if you want an automated check or local mpv build:

```bash
./scripts/ensure-vapoursynth-debian.sh
./scripts/build-mpv-vapoursynth-system.sh
```

These scripts are optional. They exist to encode the setup steps above for Debian-like systems.

### Use It

Once the checks pass, start Rhino, open a video, and enable **Preferences → Smooth video (~60 FPS at 1.0×)**. The built-in `data/vs/rhino_60_mvtools.vpy` script is used by default; choose a custom `.vpy` only if you want to replace it.

Smooth 60 runs only around **1.0×** playback speed. At 1.5× / 2.0× Rhino skips the filter. Expect higher CPU use while it is active, and a brief warm-up while the filter graph starts.

More detail: [docs/features/26-sixty-fps-motion.md](docs/features/26-sixty-fps-motion.md) and [data/vs/README.md](data/vs/README.md).

## Developer Checks

```bash
cargo test
cargo clippy --all-targets --all-features
cargo module-lines
cargo qcheck
```

The project keeps detailed feature specs and implementation notes under [docs/](docs/). Start with [docs/README.md](docs/README.md).

[vapoursynth-mvtools]: https://github.com/dubhater/vapoursynth-mvtools

## Copyright

Copyright (C) 2026 Peter Adrianov

## License

GPL-3.0-or-later (see `LICENSE`, `COPYRIGHT`, and `Cargo.toml`).
