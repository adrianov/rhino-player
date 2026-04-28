# Rhino Player — product and use cases

Rhino Player is a **desktop video and audio player**. The **product behavior** documented in `docs/features/` is portable across operating systems and UI toolkits: the same scenarios drive any implementation. Three architectural layers — product behavior, fixed core, and platform binding — are spelled out in [`docs/architecture.md`](architecture.md).

The **fixed core** is part of what Rhino is and does not change across ports: **Rust** (language), **SQLite** (persistent store), **mpv** (playback engine). The **platform binding** is everything that varies per port; today it targets **Linux** (GNOME and similar) using **GTK 4 / libadwaita** plus XDG / Freedesktop conventions. Ports to macOS or Windows replace the binding layer without rewriting feature scenarios.

This document states **why** each area matters to users and maintainers, without tying specs to any one platform binding.

## Audience and value

- **End users** want reliable playback, familiar controls, optional power features (folder queue, recent titles, streams, MPRIS), and settings that stick across sessions.
- **Integrators** (distros, power users) want clear runtime dependencies, reproducible release builds, and standard desktop integration (D-Bus, file associations).

## Use cases (by feature area)

| User need | What we implement | Spec |
|-----------|------------------|------|
| Install and build from source in a standard way | Cargo layout, documented deps, tests | [01-cargo-skeleton](features/01-cargo-skeleton.md) |
| A normal GNOME app (single instance, menu, about) | Application shell and lifecycle | [02-application-shell](features/02-application-shell.md) |
| Video and audio in the window | libmpv + OpenGL in `GLArea` | [03-mpv-embedding](features/03-mpv-embedding.md) |
| Start, stop, seek, see how long things are | Transport and progress | [04-transport-and-progress](features/04-transport-and-progress.md) |
| Watch several files in order (same folder) | Sibling / folder queue | [07-sibling-folder-queue](features/07-sibling-folder-queue.md) |
| Open from file manager, CLI, or another app | Open files and CLI | [06-open-and-cli](features/06-open-and-cli.md) |
| Pick language tracks and load external subs/audio | Tracks | [08-tracks](features/08-tracks.md) |
| Jump between chapters in long files | Chapters | [09-chapters](features/09-chapters.md) |
| Fix aspect, crop, color, sync, speed | Video options | [10-video-options](features/10-video-options.md) |
| Drop files onto the window | Drag and drop | [11-drag-and-drop](features/11-drag-and-drop.md) |
| Play URLs and network streams | URL and streams | [12-url-and-streams](features/12-url-and-streams.md) |
| Keyboard and mouse like other players | Input shortcuts | [13-input-shortcuts](features/13-input-shortcuts.md) |
| Tune defaults once | Preferences | [14-preferences](features/14-preferences.md) |
| Media keys and shell widgets | MPRIS2 | [15-mpris](features/15-mpris.md) |
| Recent titles, empty launch, resume where I left off (per file) | Continue grid + persistent store | [21-recent-videos-launch](features/21-recent-videos-launch.md) |
| Fullscreen, hide chrome, don’t lock screen while watching | Window behavior | [17-window-behavior](features/17-window-behavior.md) |
| Peek at a frame when hovering the seek bar | Thumbnail preview | [18-thumbnail-preview](features/18-thumbnail-preview.md) |
| Ship or package releases predictably | Static / release builds | [20-static-build](features/20-static-build.md) |

## Planned settings (illustrative)

Values may live in GSettings, a TOML file, or another store; the [Preferences](features/14-preferences.md) spec will fix the source of truth.

| Key (idea) | Role |
|------------|------|
| `open-new-windows` | New files open a new window vs reuse the active one |
| `normalize-volume` | Optional loudness normalization |
| `subtitle-font` / `subtitle-scale` / colors | On-screen text appearance |
| `subtitle-languages` / `audio-languages` | `slang` / `alang` for mpv |
| `hwdec` | Hardware decode on/off |
| `save-video-position` | `save-position-on-quit` |
| `volume` | Default volume |
| `show-remaining` | Elapsed vs remaining time label |
| `thumbnail-preview` | Seek-bar hover preview |
| `is-maximized` | Restore maximized state |

## Architecture and stack

The full breakdown — fixed core (Rust, SQLite, mpv), per-port binding columns (Linux today; macOS / Windows sketches), domain glossary, and boundary rules — lives in [`docs/architecture.md`](architecture.md). Feature scenarios are written in domain language and rely on the fixed core through domain terms only.

Feature details and acceptance tests stay in `docs/features/`.
