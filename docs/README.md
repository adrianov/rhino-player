# Rhino Player — documentation

Features are specified here **before** implementation, so reviewers and automated agents share one source of truth. The feature behavior is **portable**: the same scenarios must drive the current Linux / GTK / mpv / Rust implementation and any future port (e.g. macOS / AppKit / AVKit / Swift, Windows / WinUI / Media Foundation / C#). Implementation specifics live only in `## Notes`.

Each file in `docs/features/` follows the strict template defined in [`.cursor/rules/document-first.mdc`](../.cursor/rules/document-first.mdc):

1. H1 title
2. YAML front-matter (`status`, `priority`, `layers`, optional `related`, optional `scope`)
3. `## Use cases` (one-line bullets)
4. `## Description` (1–2 short paragraphs)
5. `## Behavior` (the **Gherkin** `Feature:` block — the **single source of truth** for acceptance, in domain language only)
6. `## Notes` (optional, non-binding implementation hints — the only place engine APIs, UI-toolkit widgets, OS paths, and language types may appear)

Scenarios use **Given / When / Then**. Tags, step comments, tables under **Then**, and doc strings are allowed for technical metadata; the rule file lists the controlled vocabulary and forbidden patterns.

A short **product / tree snapshot** is in the root [README](../README.md). The table below is the **index**: each row reflects the front-matter status of the linked file.

## Index

| # | Document | Status |
|---|----------|--------|
| 01 | [Cargo project and build layout](features/01-cargo-skeleton.md) | done |
| 02 | [Application shell (Adwaita app, lifecycle)](features/02-application-shell.md) | done |
| 03 | [mpv embed: render context and video surface](features/03-mpv-embedding.md) | done |
| 04 | [Transport: play, pause, seek, progress UI](features/04-transport-and-progress.md) | done |
| 05 | [Playlist: queue, prev/next, shuffle, loop](features/05-playlist.md) | planned |
| 06 | [Open files: file picker, folder, CLI, single-instance](features/06-open-and-cli.md) | wip |
| 07 | [Sibling folder queue (folder playback)](features/07-sibling-folder-queue.md) | done |
| 08 | [Tracks: audio, video, subtitles](features/08-tracks.md) | wip |
| 09 | [Chapters: marks, menu, seek bar hover](features/09-chapters.md) | planned |
| 10 | [Video options: aspect, crop, zoom, filters](features/10-video-options.md) | planned |
| 11 | [Drag and drop](features/11-drag-and-drop.md) | planned |
| 12 | [URL and network streams (yt-dlp / protocols)](features/12-url-and-streams.md) | planned |
| 13 | [Keyboard, mouse, and shortcuts](features/13-input-shortcuts.md) | done |
| 14 | [Preferences and persistent settings](features/14-preferences.md) | wip |
| 15 | [MPRIS2 (media keys, shell integration)](features/15-mpris.md) | planned |
| 16 | [Session: restore last playlist](features/16-session-persistence.md) | planned |
| 17 | [Window: size, fullscreen, UI auto-hide, inhibit idle](features/17-window-behavior.md) | wip |
| 18 | [Thumbnails: seek bar preview](features/18-thumbnail-preview.md) | done |
| 19 | [Playlist dialog (list, reorder, save m3u8)](features/19-playlist-dialog.md) | planned |
| 20 | [Static release binary and dependencies](features/20-static-build.md) | planned |
| 21 | [Recent videos grid on empty launch](features/21-recent-videos-launch.md) | done |
| 22 | [Audio: volume, mute, persistence](features/22-audio-volume-mute.md) | done |
| 23 | [Recent: continue vs done, thumbs, remove, undo (research)](features/23-recent-continue-vs-done-research.md) | research |
| 24 | [Subtitles: style, track picker, auto-pick](features/24-subtitles.md) | done |
| 25 | [Smooth video playback (display-resample) — removed](features/25-smooth-playback.md) | removed |
| 26 | [~60 fps motion (VapourSynth)](features/26-sixty-fps-motion.md) | done |
| 27 | [Move current file to trash](features/27-move-to-trash.md) | done |
| 28 | [Playback speed: 1.0× / 1.5× / 2.0×](features/28-playback-speed.md) | done |

## Tooling note (Composer 2 Fast)

Some UX targets were attempted in code but did not validate in manual testing on the maintainer’s GNOME / Wayland setup. They are documented as **not achieved in the current Cursor / Composer 2 Fast pass** (revisit with a different model or deeper GTK review): one-click switch between header `MenuButton` popovers — see [17-window-behavior](features/17-window-behavior.md).

## Architecture and product context

- [`docs/architecture.md`](architecture.md) — three-layer model (product behavior / fixed core / platform binding), domain glossary mapping scenario terms to today's core API names, and the per-port binding table.
- [`docs/product-and-use-cases.md`](product-and-use-cases.md) — audience, use-case table mapped to feature docs, planned settings.

## Technical references (upstream APIs)

- [GTK4 / GDK4: toplevel size, `compute-size`, and aspect-related notes](references-gtk4-toplevel-aspect.md)
