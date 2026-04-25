# Rhino Player — documentation

Features are specified here **before** implementation. Each feature file lives in `docs/features/` and includes name, status, use cases, short and long descriptions, and a specification.

A short **product / tree snapshot** is in the root [README](../README.md). The table below is the **index**: each **Status** cell summarizes that file; the feature doc’s **Implementation status** line is the detailed source of truth when they differ in wording.

## Index

| Document | Status |
|----------|--------|
| [Cargo project and build layout](features/01-cargo-skeleton.md) | In progress (dev builds: GTK, libadwaita, libmpv) |
| [Application shell (Adwaita app, lifecycle)](features/02-application-shell.md) | In progress (`adw` app/window, `ToolbarView`, header + sound + main menu) |
| [mpv embed: render context and video surface](features/03-mpv-embedding.md) | In progress (`GtkGLArea` + `libmpv` render) |
| [Transport: play, pause, seek, progress UI](features/04-transport-and-progress.md) | In progress (seek, times, bottom bar play/pause) |
| [Playlist: queue, prev/next, shuffle, loop](features/05-playlist.md) | Not started |
| [Open files: file picker, folder, CLI, single-instance](features/06-open-and-cli.md) | In progress (GTK “Open video” + **startup** path from `argv`; not: DnD, `HANDLES_OPEN`, folder-as-playlist, single-instance) |
| [Sibling folder queue (folder playback)](features/07-sibling-folder-queue.md) | In progress (EOF: next file in dir / next sibling) |
| [Tracks: audio, video, subtitles](features/08-tracks.md) | In progress (sound popover: audio list + `aid`; not: video / sub pickers) |
| [Chapters: marks, menu, seek bar hover](features/09-chapters.md) | Not started |
| [Video options: aspect, crop, zoom, filters](features/10-video-options.md) | Not started |
| [Drag and drop](features/11-drag-and-drop.md) | Not started |
| [URL and network streams (yt-dlp / protocols)](features/12-url-and-streams.md) | Not started |
| [Keyboard, mouse, and shortcuts](features/13-input-shortcuts.md) | In progress (Space, Escape, q, ↑/↓ volume, others — see file) |
| [Preferences and persistent settings](features/14-preferences.md) | In progress (DB `settings` + key resume paths via mpv `watch_later`) |
| [MPRIS2 (media keys, shell integration)](features/15-mpris.md) | Not started |
| [Session: restore last playlist](features/16-session-persistence.md) | Not started |
| [Window: size, fullscreen, UI auto-hide, inhibit idle](features/17-window-behavior.md) | In progress (fullscreen, `GtkWindowHandle` move-from-video, chrome autohide, cursor hide; **post-resize aspect lock not implemented** — see file) |
| [Thumbnails: seek bar preview](features/18-thumbnail-preview.md) | Not started |
| [Playlist dialog (list, reorder, save m3u8)](features/19-playlist-dialog.md) | Not started |
| [Static release binary and dependencies](features/20-static-build.md) | Not started |
| [Recent videos grid on empty launch](features/21-recent-videos-launch.md) | In progress (grid, `rhino.sqlite`, libmpv thumbs, dismiss + **Undo** bar) |
| [Audio: volume, mute, persistence](features/22-audio-volume-mute.md) | In progress (header popover, GL scroll, keys, `settings` in DB) |
| [Recent: continue vs done, thumbs, remove, undo (research / plan)](features/23-recent-continue-vs-done-research.md) | Research (deeper “finished”/DB rules TBD; partial UX lives under [21](features/21-recent-videos-launch.md)) |

## Technical references (upstream APIs)

- [GTK4 / GDK4: toplevel size, `compute-size`, and aspect-related notes (vs GTK3 `GdkGeometry`)](references-gtk4-toplevel-aspect.md)

## Product context

- **[docs/product-and-use-cases.md](product-and-use-cases.md)** — who the player is for, a use-case table mapped to feature docs, and planned settings (high level).

## Document template

Use the same sections as existing files in `docs/features/`: **name**, **implementation status**, **use cases** (user-facing value), **short description**, **long description**, **specification** (testable requirements and acceptance criteria), plus optional **current code** where implementation has started.
