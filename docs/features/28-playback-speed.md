# Playback speed (1.0× / 1.5× / 2.0×)

---
status: done
priority: p1
layers: [ui, mpv]
related: [04, 10, 26]
mpv_props: [speed, duration]
---

## Use cases
- Watch lectures or long scenes faster.
- Return to 1.0× for normal motion.

## Description
A header `MenuButton` (icon `speedometer-symbolic`) opens a popover with a `ListBox` of three rows: **1.0×**, **1.5×**, **2.0×**. Selecting a row sets mpv `speed` to that exact value, syncs the list, and closes the popover. There is no free-form slider in v1.

Speed applies to the current mpv session; mpv generally keeps `speed` across `loadfile` in the same process. The UI re-syncs from `speed` after each load and snaps to the nearest canonical step when mpv reports a value outside the trio.

## Behavior

```gherkin
@status:done @priority:p1 @layer:mpv @area:speed
Feature: Fixed-step playback speed

  Scenario: Selecting a row sets canonical speed
    Given media with measurable duration is loaded
    When the user selects 1.0×, 1.5×, or 2.0× from the header list
    Then mpv speed equals the chosen value within 0.01
    And the row highlight matches mpv speed via the re-entrancy guard

  Scenario: Snap to nearest canonical step on drift
    Given mpv reports a speed outside the canonical trio beyond 0.01
    When sync logic runs after FileLoaded
    Then mpv speed is set to the nearest of 1.0, 1.5, or 2.0
    And the list highlight matches the new speed

  Scenario: Disabled without a playable timeline
    Given duration is unavailable so the seek bar is disabled
    When the speed button reflects transport sensitivity
    Then the speed control is disabled too

  Scenario: Smooth Video coordination at non-1.0 speeds
    Given Smooth Video remains enabled in preferences
    When the user selects a non-1.0 row
    Then the VapourSynth vf is omitted while preference stays saved
    And selecting 1.0× restores the vf per 26-sixty-fps-motion rules
```

## Notes
- Read `speed` after each load; if not within 0.01 of one canonical step, set mpv to the nearest.
- Header LTR cluster: speed sits left of subtitles, volume, and the hamburger menu.
- v1 has no SQLite persistence; mpv keeps speed across `loadfile` in one run.
- `RHINO_PLAYBACK_SPEED` (used by [26-sixty-fps-motion](26-sixty-fps-motion.md)) is written from the row value before any vf rebuild.
