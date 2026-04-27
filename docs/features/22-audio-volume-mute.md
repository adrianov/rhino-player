# Audio: volume, mute, persistence

---
status: done
priority: p1
layers: [ui, mpv, db, input]
related: [08, 13, 14]
actions: []
settings: [master_volume, master_mute]
mpv_props: [volume, mute, volume-max]
---

## Use cases
- Adjust loudness without leaving the app.
- One-click mute via icon, key, or scroll wheel.
- Restore the last level on the next run.

## Description
A header **Sound** `MenuButton` with a level-aware symbolic icon opens one popover. The popover shows a single horizontal line: a circular flat mute toggle plus a horizontal scale `0..volume-max` (no numeric tick labels). When the file has at least two audio streams, an audio-track list (radio rows, scrollable) appears in the same popover (see [08-tracks](08-tracks.md)).

`volume` and `mute` are mirrored between the popover, the scroll wheel on the video surface, the keys, and mpv. Last values persist in SQLite `settings` and apply on next launch.

## Behavior

```gherkin
@status:done @priority:p1 @layer:mpv @area:audio
Feature: Volume, mute, and persistence

  Scenario: Popover and shortcuts agree with mpv
    Given the Sound popover is open
    When the user moves the scale or toggles mute
    Then mpv volume and mute change to match the chrome
    And no extra notifications are shown

  Scenario: Scroll wheel adjusts volume on the video only when chrome is visible
    Given the recent grid is hidden and the GLArea has focus
    When the user scrolls vertically on the video surface
    Then volume changes by 5% per notched step, clamped to volume-max
    And smooth-trackpad small deltas aggregate proportionally

  Scenario: Mute keeps the stored level
    Given a non-zero volume is set
    When the user toggles mute on then off
    Then the volume value is unchanged after unmute

  Scenario: Persist across quit
    Given playback ran long enough to reflect the user’s real volume and mute
    When the application quits before commit_quit stops playback
    Then SQLite stores master_volume and master_mute
    And the next launch restores them to mpv after MpvBundle creation

  Scenario: Defaults on first run
    Given no settings exist for volume or mute
    When the app starts the first time
    Then volume defaults to 100 and mute defaults to off
```

## Notes
- The scale uses `draw_value=false`; level is read from the slider and the header icon (muted / low / medium / high).
- Up / Down keys nudge volume by 5% (clamped); `m` toggles mute (see [13-input-shortcuts](13-input-shortcuts.md)).
- Persistence runs before `commit_quit` so values reflect real playback state.
