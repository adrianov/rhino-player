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

  Scenario: Preview only for local files when enabled
    Given seek_bar_preview is true and the loaded path is a local regular file
    When the user hovers the seek bar at any position
    Then a popover above the bar shows a small GL thumbnail at the hovered time
    And the centred label shows formatted hover time

  Scenario: Streams and non-file paths show no preview
    Given the loaded path is not a local regular file
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
- Hover time is computed as `(x / width) * duration` in widget coordinates.
- Popover is non-modal and arrowless; `set_pointing_to` targets a small rect just above the pointer; the `GLArea` is realised before first show.
- Thumbnail long edge clamps around 180–320 px; aspect follows current `dwidth`/`dheight`.
- Debounce 120 ms; the debounce SourceId must be taken when the timeout runs to avoid a stale-id remove later.
- The `Progress Bar Preview` row is the only preview-related preference; no separate preferences window.
- Recent grid thumbnails use `vo=image` plus DB JPEG cache via `media_probe` / `jpeg_texture`; this feature does not feed the grid.
