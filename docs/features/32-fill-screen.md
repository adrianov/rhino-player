# Fill Screen

---
status: done
priority: p1
layers: [ui, playback]
related: [17, 10]
---

## Use cases

- Eliminate black bars when the viewport aspect ratio does not match the picture.
- Eliminate black strips that are baked into the video frames themselves.
- Quickly switch between cropped-fill and fitted view in windowed or fullscreen playback.

## Description

When a video is open and the **current viewport** aspect ratio differs from the picture’s aspect ratio **after baked-in black strips are removed** (or from the full frame when strips are not detected), a **Fill Screen** button appears in the header bar. Activating it zooms and crops so the picture covers the viewport: letterboxing/pillarboxing is removed by fill-zoom, and baked-in strips are cropped out so they leave the viewport. The button acts as a toggle; tapping again restores the fitted view. The button is hidden when the viewport already matches that content aspect (no useful fill).

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui
Feature: Fill Screen

  Background:
    Given a video is playing

  Scenario: Fill button visible on viewport aspect mismatch
    Given the viewport aspect ratio does not match the picture aspect ratio
    When the player shows the video
    Then the Fill Screen button is visible in the header bar
    And the button is not in the active state

  Scenario: Fill button visible in windowed mode
    Given the player is not in fullscreen
    And the viewport aspect ratio does not match the picture aspect ratio
    When the player shows the video
    Then the Fill Screen button is visible in the header bar

  Scenario: Fill button visible when strips change the content aspect
    Given the full-frame aspect ratio matches the viewport
    And the frames contain detectable black strips
    And removing those strips yields a different aspect ratio than the viewport
    When strip detection finishes
    Then the Fill Screen button is visible in the header bar

  Scenario: Fill button hidden when viewport matches content aspect
    Given the viewport aspect ratio matches the picture aspect ratio after any strip removal
    When the player shows the video
    Then the Fill Screen button is not visible

  Scenario: Activate fill
    Given the Fill Screen button is visible and inactive
    When the user clicks the Fill Screen button
    Then the video zooms to fill the entire viewport
    And the button changes to the active state

  Scenario: Activate fill crops baked-in strips
    Given the Fill Screen button is visible because strip removal changes the content aspect
    And the button is inactive
    When the user clicks the Fill Screen button
    Then the picture enlarges so the black strips leave the viewport
    And the button changes to the active state

  Scenario: Deactivate fill
    Given the Fill Screen button is in the active state
    When the user clicks the Fill Screen button
    Then the video returns to the fitted (letterboxed/pillarboxed) view
    And baked-in strips are visible again if present in the frames
    And the button returns to the inactive state

  Scenario: Fill follows fullscreen exit when windowed viewport still mismatches
    Given the Fill Screen button is in the active state in fullscreen
    When the user exits fullscreen
    And the windowed viewport aspect still differs from the content aspect
    Then the filled view remains
    And the Fill Screen button stays visible and active

  Scenario: Fill resets when the viewport matches content aspect
    Given the Fill Screen button is in the active state
    When the viewport aspect ratio matches the content aspect ratio
    Then the fitted view is restored
    And the button is no longer visible

  Scenario: Fill resets on new media
    Given the Fill Screen button is in the active state
    When a new video starts playing
    Then the fitted view is restored
    And the button visibility reflects the new video's content aspect and the viewport

  Scenario: Fill choice remembered for its video
    Given the Fill Screen button was activated for a video
    When that same video is opened again with a viewport aspect mismatch
    Then the video automatically fills the viewport
    And the button appears in the active state

  Scenario: Explicit fitted choice also remembered
    Given the user had switched fill off for a video while it could fill
    When that same video is opened again with a viewport aspect mismatch
    Then the video shows the fitted view with the button inactive

  Scenario: Videos without a stored choice open fitted
    Given a video has never had its Fill Screen button toggled
    When it plays with a viewport aspect mismatch
    Then the video shows the fitted view until the user activates fill
```

## Notes

- Implemented in `src/video_fill.rs` (+ `fill_sync`); baked-in strips owned by `src/black_bars` (packed `frame` + lavfi `probe`).
- Aspect fill uses mpv `panscan`: `0.0` = fitted (default), `1.0` = fills the video surface, crops symmetrically.
- Baked-in strips: temporary labeled `cropdetect` vf (FFmpeg lavfi), then mpv `video-crop` (`WxH+X+Y`) while Fill is on; cleared when Fill is off or media changes. Probe timing / shared crop guards live in `black_bars` (`DETECT_DELAY`, `pump_bar_probe` on reconfig, `READY_RETRY` / `READY_RETRY_MAX` fallback). Metadata via `MPV_FORMAT_NODE` on `vf-metadata/<label>` only (`read_meta_node`) — not per-key `lavfi.cropdetect.*` props (libmpv NULL-tags SIGSEGV).
- Non-copy hardware decode is paused for the probe only (same idea as mpv `autocrop.lua`); restored afterward.
- Aspect ratio tolerance constant in `src/video_fill.rs` (`AR_TOLERANCE`).
- Viewport aspect from the video surface widget size (`GLArea`); content aspect from strip `CropRect` when known, else mpv `dwidth` / `dheight`.
- Button icon: `view-fill-symbolic` (`data/icons/hicolor/scalable/actions/view-fill-symbolic.svg`).
- Button visibility is refreshed by `video_fill::request_fill_resync()` from `VideoReconfig` and `FileLoaded`, on fullscreen changes, and on video-surface resize after `bind_fill_viewport`; strip probe starts from FileLoaded / path reset.
- Fill choice persists per video in `media.fill_screen` (`db::media_fill_screen` /
  `db::media_save_fill_screen`); written only on an explicit button toggle, restored on media open when the viewport can fill.
