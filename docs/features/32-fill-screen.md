# Fill Screen

---
status: done
priority: p1
layers: [ui, playback]
related: [17, 10]
---

## Use cases

- Eliminate black bars when watching content whose aspect ratio doesn't match the display.
- Eliminate black strips that are baked into the video frames themselves.
- Quickly switch between cropped-fill and letterboxed/pillarboxed view without leaving fullscreen.

## Description

When the player is in fullscreen and either the video aspect ratio differs from the screen aspect ratio or the frames contain detectable black strips, a **Fill Screen** button appears in the header bar. Activating it zooms and crops so the picture covers the screen: container letterboxing/pillarboxing is removed by fill-zoom, and baked-in black strips are cropped out so they leave the viewport. The button acts as a toggle; tapping again restores the original fitted view. The button is hidden in windowed mode and also hidden when the video already matches the screen with no detectable strips.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui
Feature: Fill Screen

  Background:
    Given the player is in fullscreen mode
    And a video is playing

  Scenario: Fill button visible on aspect mismatch
    Given the video aspect ratio does not match the screen aspect ratio
    When the player enters fullscreen
    Then the Fill Screen button is visible in the header bar
    And the button is not in the active state

  Scenario: Fill button visible when baked-in strips are detected
    Given the video aspect ratio matches the screen aspect ratio
    And the frames contain detectable black strips
    When strip detection finishes
    Then the Fill Screen button is visible in the header bar

  Scenario: Fill button hidden when aspects match and no strips
    Given the video aspect ratio matches the screen aspect ratio
    And the frames do not contain detectable black strips
    When the player enters fullscreen
    Then the Fill Screen button is not visible

  Scenario: Activate fill
    Given the Fill Screen button is visible and inactive
    When the user clicks the Fill Screen button
    Then the video zooms to fill the entire screen
    And the button changes to the active state

  Scenario: Activate fill crops baked-in strips
    Given the Fill Screen button is visible because of detectable black strips
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

  Scenario: Fill resets on fullscreen exit
    Given the Fill Screen button is in the active state
    When the user exits fullscreen
    Then the fitted view is restored
    And the button is no longer visible

  Scenario: Fill resets on new media
    Given the Fill Screen button is in the active state
    When a new video starts playing
    Then the fitted view is restored
    And the button visibility reflects the new video's aspect ratio and strip detection

  Scenario: Fill choice remembered for its video
    Given the Fill Screen button was activated for a video
    When that same video is opened again and the player enters fullscreen
    Then the video automatically fills the entire screen
    And the button appears in the active state

  Scenario: Explicit fitted choice also remembered
    Given the user had switched fill off for a video while it could fill
    When that same video is opened again and the player enters fullscreen
    Then the video shows the fitted view with the button inactive

  Scenario: Videos without a stored choice open fitted
    Given a video has never had its Fill Screen button toggled
    When it plays in fullscreen with an aspect mismatch or detectable strips
    Then the video shows the fitted view until the user activates fill
```

## Notes

- Implemented in `src/video_fill.rs` (+ `fill_sync`); baked-in strips owned by `src/black_bars` (packed `frame` + lavfi `probe`).
- Aspect fill uses mpv `panscan`: `0.0` = fitted (default), `1.0` = fills screen, crops symmetrically.
- Baked-in strips: temporary labeled `cropdetect` vf (FFmpeg lavfi), then mpv `video-crop` (`WxH+X+Y`) while Fill is on; cleared when Fill is off or media changes. Probe timing / shared crop guards live in `black_bars` (`DETECT_DELAY`, `pump_bar_probe` on reconfig, `READY_RETRY` / `READY_RETRY_MAX` fallback).
- Non-copy hardware decode is paused for the probe only (same idea as mpv `autocrop.lua`); restored afterward.
- Aspect ratio tolerance constant in `src/video_fill.rs` (`AR_TOLERANCE`).
- Video dimensions read from mpv `dwidth` / `dheight` properties; screen dimensions from the GDK monitor geometry.
- Button icon: `view-fill-symbolic` (`data/icons/hicolor/scalable/actions/view-fill-symbolic.svg`).
- Button visibility is refreshed by `video_fill::request_fill_resync()` from `VideoReconfig` and `FileLoaded` transport events; strip probe starts from FileLoaded / path reset.
- Fill choice persists per video in `media.fill_screen` (`db::media_fill_screen` /
  `db::media_save_fill_screen`); written only on an explicit button toggle, restored on media open.
