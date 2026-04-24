# Rhino Player — documentation

Features are specified here **before** implementation. Each feature file lives in `docs/features/` and includes name, status, use cases, short and long descriptions, and a specification.

## Index

| Document | Status |
|----------|--------|
| [Cargo project and build layout](features/01-cargo-skeleton.md) | In progress (GTK/mpv build OK) |
| [Application shell (Adwaita app, lifecycle)](features/02-application-shell.md) | In progress (`adw` app/window, `ToolbarView`, menu) |
| [mpv embed: render context and video surface](features/03-mpv-embedding.md) | In progress (GLArea + `libmpv` render) |
| [Transport: play, pause, seek, progress UI](features/04-transport-and-progress.md) | Not started |
| [Playlist: queue, prev/next, shuffle, loop](features/05-playlist.md) | Not started |
| [Open files: file picker, folder, CLI, single-instance](features/06-open-and-cli.md) | Not started |
| [Sibling folder queue (folder playback)](features/07-sibling-folder-queue.md) | Not started |
| [Tracks: audio, video, subtitles](features/08-tracks.md) | Not started |
| [Chapters: marks, menu, seek bar hover](features/09-chapters.md) | Not started |
| [Video options: aspect, crop, zoom, filters](features/10-video-options.md) | Not started |
| [Drag and drop](features/11-drag-and-drop.md) | Not started |
| [URL and network streams (yt-dlp / protocols)](features/12-url-and-streams.md) | Not started |
| [Keyboard, mouse, and shortcuts](features/13-input-shortcuts.md) | Not started |
| [Preferences and persistent settings](features/14-preferences.md) | Not started |
| [MPRIS2 (media keys, shell integration)](features/15-mpris.md) | Not started |
| [Session: restore last playlist](features/16-session-persistence.md) | Not started |
| [Window: size, fullscreen, UI auto-hide, inhibit idle](features/17-window-behavior.md) | Not started |
| [Thumbnails: seek bar preview](features/18-thumbnail-preview.md) | Not started |
| [Playlist dialog (list, reorder, save m3u8)](features/19-playlist-dialog.md) | Not started |
| [Static release binary and dependencies](features/20-static-build.md) | Not started |

## Product context

- **[docs/product-and-use-cases.md](product-and-use-cases.md)** — who the player is for, a use-case table mapped to feature docs, and planned settings (high level).

## Document template

Use the same sections as existing files in `docs/features/`: **name**, **implementation status**, **use cases** (user-facing value), **short description**, **long description**, **specification** (testable requirements and acceptance criteria), plus optional **current code** where implementation has started.
