# Thumbnails: seek bar preview

---
status: done
priority: p1
layers: [ui, mpv, db]
related: [03, 04, 14, 21]
settings: [seek_bar_preview]
mpv_props: [vo-configured, time-pos, duration, path, dwidth, dheight]
---

## Use cases
- Scrub the timeline visually before seeking, especially on long local files.

## Description
On hover over the bottom seek scale, a popover above the bar shows a framed live video thumbnail with a small centred time label. The preview uses a second in-process libmpv with `vo=libmpv` and the same OpenGL render path as the main embed (`MpvPreviewGl`). Hover motion is debounced (~120 ms) and seeks the preview player with `seek absolute+keyframes`. The toggle is **Progress Bar Preview** in the main menu Preferences.

The auxiliary preview is video-only: `ao=null`, `pause=yes`, `aid=no`, `sid=no`, no external autoload, no scripts/config, no resume, small demuxer cache, fast decoder flags, and it does not copy main-player track or `hwdec` selections. VapourSynth and bundled `.vpy` apply only to the main player.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui @area:preview
Feature: Seek bar thumbnail preview

  Scenario: Preview for openable local media when enabled
    Given seek bar preview is on and the open item is a local file or disc tree the shell opened
    When the user hovers the seek bar at any position
    Then a popover above the bar shows a small GL thumbnail at the hovered time
    And the centred label shows formatted hover time

  Scenario: Streams and non-openable paths show no preview
    Given the open item is not local openable media (e.g. a remote stream)
    When the user hovers the seek bar
    Then no popover appears
    And no preview seek runs

  Scenario: Debounced seeks avoid pile-up
    Given the user moves the pointer quickly along the seek bar
    When debounced preview seeks run
    Then only the latest hover position issues a seek
    And cancelled debounce timers do not affect later positions

  Scenario: Single auxiliary player at a time
    Given preview is active during a session
    When the popover is visible repeatedly
    Then only one MpvPreviewGl instance exists at any time

  Scenario: Toggle off hides preview chrome only
    Given seek_bar_preview is false
    When the user hovers the seek bar
    Then no thumbnail appears
    And transport remains usable
```

## Notes
- Settings: SQLite `seek_bar_preview` defaults to **on**; toggled from main menu Preferences (gio stateful action `seek-bar-preview`).
- Hover time is `(x / width) * bar_upper` capped by [seek_bar_label_time]. Pointer release on the seek bar (trough or thumb drag) seeks the main player to that hover time, not the raw GtkRange thumb value; preview off falls back to capped thumb time ([`seek_wiring`](../../src/app/seek_wiring.rs)).
- Preview **`GtkFrame`** on **`outer_ovl`** above the bottom bar; positioned from seek-bar pointer x; the preview **`GLArea`** is realised before first show.
- Thumbnail long edge clamps around 180–320 px; aspect follows current `dwidth`/`dheight`.
- Debounce 120 ms; trailing coalesce (motion updates hover time without resetting the timer). Debounce/pump run at default GLib priority so they are not starved during DVD bar work.
- The `Progress Bar Preview` row is the only preview-related preference; no separate preferences window.
- Recent grid thumbnails use `vo=image` plus DB JPEG cache via `media_probe` / `jpeg_texture`; this feature does not feed the grid.
- Load target: prefer main mpv `path` when it is a local stream file (e.g. Blu-ray `.m2ts` under `STREAM/`) or `bd://` / `bluray://`; else `shell_media_path` + `resolve_open_media_path`. Preview always uses software decode (`hwdec=no`); DVD `.vob` uses `hr-seek=yes` on the preview player only. Preview always `vf clr` before `loadfile` and `keep-open=always` so EOF does not unload. Hover/seek times use the minimum of seek-bar upper, main `duration`, and preview `duration`, capped ~1–4 s before the end. Debounced seeks use `absolute+exact` on optical / `absolute+keyframes` otherwise; a frame pump waits for `vo-configured` then renders (scrub reuses the same pump). On main media change, preview cache and owner entity reset without touching the preview GL context (avoids stealing the main embed context on macOS). Debug: `[rhino] preview:` lifecycle lines always on plain `cargo run`; `RHINO_PREVIEW_DEBUG=1` adds per-tick pump detail; reset sites log `reset from <caller>` (`preview_debug.rs`).
- UI: preview is a **`GtkFrame`** overlay on **`outer_ovl`** (not **`GtkPopover`**) — no separate compositor surface. **macOS theater:** `seek_bar_preview/macos_compositing.rs` — widget-level opaque CSS (gray frame `#2d2d2d`, black preview **`GLArea`**), **`raise_overlay_child`**, **`on_overlay_surface_opened`** / **`on_menu_surface_closed`** on hide; see [`references-gtk4-macos-header-menus.md`](../references-gtk4-macos-header-menus.md) (**Theater overlay compositing**).
