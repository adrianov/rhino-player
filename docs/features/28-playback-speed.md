# Playback speed (fixed steps)

---
status: done
priority: p1
layers: [ui, mpv]
related: [04, 07, 10, 13, 26]
mpv_props: [speed, duration]
---

## Use cases
- Watch lectures or long scenes faster.
- Return to 1.0× for normal motion.

## Description
A header `MenuButton` (icon `speedometer-symbolic`) opens a popover with a `ListBox` of fixed-rate rows (**1.0×**, **1.5×**, **2.0×**, integers **3.0×** through **8.0×**). A compact caption readout sits immediately under that icon within the same header slot—without shifting the icon out of alignment with neighbouring controls—while media is loaded. Keyboard digits **1**–**8** jump to fixed rates (**13-input-shortcuts**): **3** selects **1.5×**; **1**, **2**, and **4**–**8** select that many times normal speed. Selecting a row sets mpv `speed` to that exact value, syncs the list, and closes the popover. There is no free-form slider in v1.

Speed applies to the current mpv session; mpv generally keeps `speed` across `loadfile` in the same process, except automatic advance to the next file in folder order resets to **1.0×** before the new file loads. The UI re-syncs from `speed` after each load and snaps to the nearest canonical step when mpv reports a value outside the fixed set.

## Behavior

```gherkin
@status:done @priority:p1 @layer:mpv @area:speed
Feature: Fixed-step playback speed

  Scenario: Selecting a row sets canonical speed
    Given media with measurable duration is loaded
    When the user selects any fixed rate from the header list
    Then mpv speed equals the chosen value within 0.01
    And the compact header speed readout shows the chosen fixed rate
    And the row highlight matches mpv speed via the re-entrancy guard

  Scenario: Digit keys match fixed list rates
    Given media with measurable duration is loaded
    When the user chooses playback rate using keyboard shortcut digits one through eight
    Then playback speed matches the fixed-rate shortcut for that digit within 0.01
    And the compact header speed readout shows the chosen fixed rate
    And the header list highlight matches the same canonical step

  Scenario: Snap to nearest canonical step on drift
    Given mpv reports a speed outside the canonical fixed steps beyond 0.01
    When sync logic runs after FileLoaded
    Then mpv speed is set to the nearest canonical step
    And the list highlight matches the new speed
    And the compact header speed readout matches the new canonical step

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
- Compact header speed readout: **`rp-speed-readout`** stacked immediately under the speed `MenuButton` in **`rp-speed-cluster`** (`Vertical`/`spacing 0`; CSS **`margin-top: -8px`** on the caption). Icon stays centred in-cluster like a lone **`MenuButton`**. Disabled on the continue grid (`duration` unavailable); transport tick keeps it aligned with the canonical step (`dispatch_sync_ui` / `playback_speed`).
- Digit **3** sets **1.5×**; digits **1**, **2**, **4**–**8** set **N**× (**13-input-shortcuts**).
- Read `speed` after each load; if not within 0.01 of one canonical step, set mpv to the nearest.
- Header LTR cluster: speed sits left of subtitles, volume, and the hamburger menu.
- v1 has no SQLite persistence; mpv keeps speed across `loadfile` in one run except sibling-folder auto-advance (see [07-sibling-folder-queue](07-sibling-folder-queue.md)).
- `RHINO_PLAYBACK_SPEED` (used by [26-sixty-fps-motion](26-sixty-fps-motion.md)) is written from the row value before any vf rebuild.
