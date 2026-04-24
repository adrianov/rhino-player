# Rhino Player

A media player for Linux (GNOME, Ubuntu, and similar) that uses **mpv** for decoding and **GTK 4** with **libadwaita** for the user interface. The implementation language is **Rust**. The long-term goal is a reliable distributable, with **static** or mostly self-contained release artifacts where the platform allows; see [docs/features/20-static-build.md](docs/features/20-static-build.md).

## Features (planned)

Planned behavior is specified under `docs/features/` and summarized by user need in [docs/product-and-use-cases.md](docs/product-and-use-cases.md). High-level areas:

- mpv embed via `libmpv` and OpenGL (GTK `GLArea`) on X11 and Wayland
- Playlist, shuffle, loop, folder / sibling queue
- Subtitles, audio, and video track selection; chapters
- Video options (aspect, crop, color adjustments, speed, A/V delay)
- Drag and drop, URL / stream opening (yt-dlp)
- MPRIS2, session restore, preferences, global shortcuts, optional seek-bar thumbnails

## Document-first development

We specify behavior in `docs/` before (or in step with) implementation. The index is [docs/README.md](docs/README.md). Cursor / agent rules in `.cursor/rules/document-first.mdc` enforce this for AI-assisted work.

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

You can open a local media file from the menu (**Open…**, Ctrl+O). Video is drawn via `vo=libmpv` into a `GtkGLArea` (OpenGL + EGL). Audio is requested with `ao=pulse` (works with PipeWire’s PulseAudio layer on many setups). If the picture is black or there is no sound, check `mpv`’s `hwdec` / `ao` in future preferences.

## License

GPL-3.0-or-later (intended, matching common GNOME/mpv ecosystem choices — confirm in `Cargo.toml` before first release if different).
