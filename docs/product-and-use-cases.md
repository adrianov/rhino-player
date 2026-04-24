# Rhino Player — product and use cases

Rhino Player is a **desktop video and audio player** for Linux (GNOME and similar) that uses **mpv** for playback and **GTK 4 / libadwaita** for the interface, implemented in **Rust**. This document states **why** each area matters to users and maintainers, without tying specs to any other application.

## Audience and value

- **End users** want reliable playback, familiar controls, optional power features (playlists, streams, MPRIS), and settings that stick across sessions.
- **Integrators** (distros, power users) want clear runtime dependencies, reproducible release builds, and standard desktop integration (D-Bus, file associations).

## Use cases (by feature area)

| User need | What we implement | Spec |
|-----------|------------------|------|
| Install and build from source in a standard way | Cargo layout, documented deps, tests | [01-cargo-skeleton](features/01-cargo-skeleton.md) |
| A normal GNOME app (single instance, menu, about) | Application shell and lifecycle | [02-application-shell](features/02-application-shell.md) |
| Video and audio in the window | libmpv + OpenGL in `GLArea` | [03-mpv-embedding](features/03-mpv-embedding.md) |
| Start, stop, seek, see how long things are | Transport and progress | [04-transport-and-progress](features/04-transport-and-progress.md) |
| Watch several files in order; shuffle/loop | Playlist | [05-playlist](features/05-playlist.md) |
| Open from file manager, CLI, or another app | Open files and CLI | [06-open-and-cli](features/06-open-and-cli.md) |
| “Play the whole album/folder in this directory” from one file | Sibling / folder queue | [07-sibling-folder-queue](features/07-sibling-folder-queue.md) |
| Pick language tracks and load external subs/audio | Tracks | [08-tracks](features/08-tracks.md) |
| Jump between chapters in long files | Chapters | [09-chapters](features/09-chapters.md) |
| Fix aspect, crop, color, sync, speed | Video options | [10-video-options](features/10-video-options.md) |
| Drop files onto the window | Drag and drop | [11-drag-and-drop](features/11-drag-and-drop.md) |
| Play URLs and network streams | URL and streams | [12-url-and-streams](features/12-url-and-streams.md) |
| Keyboard and mouse like other players | Input shortcuts | [13-input-shortcuts](features/13-input-shortcuts.md) |
| Tune defaults once | Preferences | [14-preferences](features/14-preferences.md) |
| Media keys and shell widgets | MPRIS2 | [15-mpris](features/15-mpris.md) |
| Resume where I left off (playlist / position) | Session | [16-session-persistence](features/16-session-persistence.md) |
| Fullscreen, hide chrome, don’t lock screen while watching | Window behavior | [17-window-behavior](features/17-window-behavior.md) |
| Peek at a frame when hovering the seek bar | Thumbnail preview | [18-thumbnail-preview](features/18-thumbnail-preview.md) |
| See and edit the full queue in a list | Playlist dialog | [19-playlist-dialog](features/19-playlist-dialog.md) |
| Ship or package releases predictably | Static / release builds | [20-static-build](features/20-static-build.md) |
| Open the app with no file and quickly resume recent media | Recent videos grid (empty launch) | [21-recent-videos-launch](features/21-recent-videos-launch.md) |

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
| `save-session` | Write last playlist on exit |
| `is-maximized` | Restore maximized state |

## Technology scope (this project)

- **UI:** GTK 4, libadwaita.
- **Playback:** libmpv; embedding via render API and OpenGL.
- **Platform:** D-Bus (MPRIS), XDG config paths, Pulse/PipeWire audio as typical on target desktops.
- **Build:** Rust and Cargo; release strategy is defined in the static-build feature (dynamic system libs, optional bundling—no prescriptive store format in this document).

Feature details and acceptance tests stay in `docs/features/`.
