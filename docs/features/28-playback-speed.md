# Playback speed (fixed steps)

---
status: done
priority: p1
layers: [ui, mpv]
related: [04, 07, 10, 26]
mpv_props: [speed, duration]
---

## Use cases
- Watch lectures or long scenes faster.
- Return to 1.0× for normal motion.

## Description
A header `MenuButton` (icon `speedometer-symbolic`) opens a popover with a `ListBox` of four rows: **1.0×**, **1.5×**, **2.0×**, and **8.0×** (fast skip through dull segments). Selecting a row sets mpv `speed` to that exact value, syncs the list, and closes the popover. There is no free-form slider in v1.

Speed applies to the current mpv session; mpv generally keeps `speed` across `loadfile` in the same process, except automatic advance to the next file in folder order resets to **1.0×** before the new file loads. The UI re-syncs from `speed` after each load and snaps to the nearest canonical step when mpv reports a value outside the fixed set.

## Behavior

```gherkin
@status:done @priority:p1 @layer:mpv @area:speed
Feature: Fixed-step playback speed

  Scenario: Selecting a row sets canonical speed
    Given media with measurable duration is loaded
    When the user selects 1.0×, 1.5×, 2.0×, or 8.0× from the header list
    Then mpv speed equals the chosen value within 0.01
    And the row highlight matches mpv speed via the re-entrancy guard

  Scenario: Snap to nearest canonical step on drift
    Given mpv reports a speed outside the canonical fixed steps beyond 0.01
    When sync logic runs after FileLoaded
    Then mpv speed is set to the nearest canonical step
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

  Scenario: Folder auto-advance restores normal speed
    Given the playback rate is faster than normal
    When the current file ends and the next file in folder order loads automatically
    Then the playback rate is normal (1.0×)
    And the speed list shows normal speed
```

## Notes
- Fastest row **8.0×** matches mpv default audio pitch preservation: auto `scaletempo2` uses `max-speed=8.0` upstream, so higher `speed` values do not apply reliably with default options.
- Read `speed` after each load; if not within 0.01 of one canonical step, set mpv to the nearest.
- Header LTR cluster: speed sits left of subtitles, volume, and the hamburger menu.
- v1 has no SQLite persistence; mpv keeps speed across `loadfile` in one run except sibling-folder auto-advance (see [07-sibling-folder-queue](07-sibling-folder-queue.md)).
- `RHINO_PLAYBACK_SPEED` (used by [26-sixty-fps-motion](26-sixty-fps-motion.md)) is written from the row value before any vf rebuild.
