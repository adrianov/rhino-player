# Rhino Player — documentation

Features are specified here **before** implementation. Each feature file lives in `docs/features/` and includes name, status, use cases, short and long descriptions, and a specification.

A short **product / tree snapshot** is in the root [README](../README.md). The table below is the **index**: each **Status** cell summarizes that file; the feature doc’s **Implementation status** line is the detailed source of truth when they differ in wording.

## Index

| Document | Status |
|----------|--------|
| [Cargo project and build layout](features/01-cargo-skeleton.md) | **Done** (repo layout, `cargo build` / `cargo test`; static release: [20](features/20-static-build.md)) |
| [Application shell (Adwaita app, lifecycle)](features/02-application-shell.md) | **Done** (`adw` app/window, `ToolbarView`, header + sound + main menu, open / **Close Video** / **Move to Trash** (local file) / about / quit, dark; not: full preferences window, `open` for remote files — see [06](features/06-open-and-cli.md)) |
| [mpv embed: render context and video surface](features/03-mpv-embedding.md) | **Done** (`GtkGLArea` + `libmpv` render, watch-later dir, resume) |
| [Transport: play, pause, seek, progress UI](features/04-transport-and-progress.md) | **Done** (seek, times, bottom bar; prev/next per sibling order — not: shuffle/loop from [05](features/05-playlist.md)) |
| [Playback speed: 1.0x / 1.5x / 2.0x](features/28-playback-speed.md) | **Done** (header `speedometer-symbolic` + list popover; left of other header popovers; libmpv `speed`) |
| [Playlist: queue, prev/next, shuffle, loop](features/05-playlist.md) | Not started |
| [Open files: file picker, folder, CLI, single-instance](features/06-open-and-cli.md) | In progress (GTK “Open Video” + **startup** path from `argv`; not: DnD, `HANDLES_OPEN`, folder-as-playlist, single-instance) |
| [Sibling folder queue (folder playback)](features/07-sibling-folder-queue.md) | **Done** (EOF + bottom **Prev/Next** with filename tooltips, `sibling_advance`; **not:** m3u playlist UI) |
| [Tracks: audio, video, subtitles](features/08-tracks.md) | In progress (sound: **Audio** + `aid`; [Subtitles](features/24-subtitles.md) popover) |
| [Subtitles: style, track picker, auto-pick](features/24-subtitles.md) | **Done** (header button, `sub-*` strings, DB + last-pick Levenshtein) |
| [Smooth video playback (display-resample) — removed](features/25-smooth-playback.md) | **Removed** (superseded by [26](features/26-sixty-fps-motion.md)) |
| [~60 fps motion (VapourSynth)](features/26-sixty-fps-motion.md) | **Done** (menu **Preferences** → **Smooth Video (~60 FPS at 1.0×)**; `video_smooth_60`, `video_vs_path`, `video_mvtools_lib` cache; bundled `data/vs/*.vpy`) |
| [Move current file to trash](features/27-move-to-trash.md) | **Done** (main menu **Move to Trash**; `app.move-to-trash`; [Delete] / [KP_Delete] when a local file is playing) |
| [Chapters: marks, menu, seek bar hover](features/09-chapters.md) | Not started |
| [Video options: aspect, crop, zoom, filters](features/10-video-options.md) | Not started |
| [Drag and drop](features/11-drag-and-drop.md) | Not started |
| [URL and network streams (yt-dlp / protocols)](features/12-url-and-streams.md) | Not started |
| [Keyboard, mouse, and shortcuts](features/13-input-shortcuts.md) | **Done (in-app)** (Space, Escape, fullscreen, RMB pause, `m`, arrows, `q`; not: full mpv `input.conf` forward, global Shortcuts help) |
| [Preferences and persistent settings](features/14-preferences.md) | In progress (DB `settings` + mpv `watch_later` / resume; **not:** preferences dialog / GSettings schema) |
| [MPRIS2 (media keys, shell integration)](features/15-mpris.md) | Not started |
| [Session: restore last playlist](features/16-session-persistence.md) | Not started |
| [Window: size, fullscreen, UI auto-hide, inhibit idle](features/17-window-behavior.md) | In progress (fullscreen, `GtkWindowHandle`, chrome autohide, cursor hide, fit-on-open, inhibit; **not:** post-resize aspect lock, **one-click header menu switch** — see file) |
| [Thumbnails: seek bar preview](features/18-thumbnail-preview.md) | **Done** (`seek_bar_preview` + `MpvPreviewGl`: second `vo=libmpv` in popover `GLArea`) |
| [Playlist dialog (list, reorder, save m3u8)](features/19-playlist-dialog.md) | Not started |
| [Static release binary and dependencies](features/20-static-build.md) | Not started |
| [Recent videos grid on empty launch](features/21-recent-videos-launch.md) | **Done** (grid, `rhino.sqlite`, libmpv thumbs, dismiss, **session** undo stack) |
| [Audio: volume, mute, persistence](features/22-audio-volume-mute.md) | **Done** (header popover, GL scroll, keys, `settings` in DB) |
| [Recent: continue vs done, thumbs, remove, undo (research / plan)](features/23-recent-continue-vs-done-research.md) | Research (deeper “finished”/DB rules TBD; partial UX lives under [21](features/21-recent-videos-launch.md)) |

## Tooling note (Composer 2 Fast)

Some UX targets were **attempted in code** but **did not validate in manual testing** on the maintainer’s GNOME/Wayland setup. They are documented as **not achieved in the current Cursor / Composer 2 Fast pass** (revisit with a different model or deeper GTK review): **header menu popovers** switching with a single click (see [17-window-behavior](features/17-window-behavior.md)).

## Technical references (upstream APIs)

- [GTK4 / GDK4: toplevel size, `compute-size`, and aspect-related notes (vs GTK3 `GdkGeometry`)](references-gtk4-toplevel-aspect.md)

## Product context

- **[docs/product-and-use-cases.md](product-and-use-cases.md)** — who the player is for, a use-case table mapped to feature docs, and planned settings (high level).

## Document template

Use the same sections as existing files in `docs/features/`: **name**, **implementation status**, **use cases** (user-facing value), **short description**, **long description**, **specification** (testable requirements and acceptance criteria), plus optional **current code** where implementation has started.
