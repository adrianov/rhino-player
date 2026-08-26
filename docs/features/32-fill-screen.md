# Fill Screen

---
status: done
priority: p1
layers: [ui, playback]
related: [17, 10]
---

## Use cases

- Eliminate black bars when watching content whose aspect ratio doesn't match the display.
- Quickly switch between cropped-fill and letterboxed/pillarboxed view without leaving fullscreen.

## Description

When the player is in fullscreen and the video aspect ratio differs from the screen aspect ratio, a **Fill Screen** button appears in the header bar. Activating it zooms the video to cover the entire screen, cropping the central region symmetrically. The button acts as a toggle; tapping again restores the original fitted view. The button is hidden in windowed mode and also hidden when the video already matches the screen aspect ratio.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui
Feature: Fill Screen

  Background:
    Given the player is in fullscreen mode
    And a video is playing whose aspect ratio differs from the screen

  Scenario: Fill button visible on aspect mismatch
    Given the video aspect ratio does not match the screen aspect ratio
    When the player enters fullscreen
    Then the Fill Screen button is visible in the header bar
    And the button is not in the active state

  Scenario: Fill button hidden when aspects match
    Given the video aspect ratio matches the screen aspect ratio
    When the player enters fullscreen
    Then the Fill Screen button is not visible

  Scenario: Activate fill
    Given the Fill Screen button is visible and inactive
    When the user clicks the Fill Screen button
    Then the video zooms to fill the entire screen
    And the button changes to the active state

  Scenario: Deactivate fill
    Given the Fill Screen button is in the active state
    When the user clicks the Fill Screen button
    Then the video returns to the fitted (letterboxed/pillarboxed) view
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
    And the button visibility reflects the new video's aspect ratio

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
    When it plays in fullscreen with an aspect mismatch
    Then the video shows the fitted view until the user activates fill
```

## Notes

- Implemented in `src/video_fill.rs`.
- Uses mpv `panscan` property: `0.0` = fitted (default), `1.0` = fills screen, crops symmetrically.
- Aspect ratio tolerance constant in `src/video_fill.rs` (`AR_TOLERANCE`).
- Video dimensions read from mpv `dwidth` / `dheight` properties; screen dimensions from the GTK window allocation in fullscreen.
- Button icon: `view-fill-symbolic` (`data/icons/hicolor/scalable/actions/view-fill-symbolic.svg`).
- Button visibility is refreshed by `video_fill::request_fill_resync()` from `VideoReconfig` and `FileLoaded` transport events.
- Fill choice persists per video in `media.fill_screen` (`db::media_fill_screen` /
  `db::media_save_fill_screen`); written only on an explicit button toggle, restored on media open.
