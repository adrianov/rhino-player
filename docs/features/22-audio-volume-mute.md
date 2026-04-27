# Audio: volume, mute, persistence

**Name:** System volume, mute, and last level persistence

**Implementation status:** Done

**Use cases:** Adjust loudness without leaving the app; one-click mute; match expectations from other desktop players and mpv; restore last used level on the next run.

**Short description:** Header **Sound** control with one popover: one **horizontal line** (mute icon toggle + scale, no extra labels) and, when applicable, an **audio track** list without a section title (see [08-tracks](08-tracks.md)); scroll-wheel on the video, keyboard shortcuts, and `volume` / `mute` in the app database for the next run.

**Long description:** A `MenuButton` with a symbolic icon (muted / low / medium / high) opens a popover: **one line** (no numeric tick labels on the scale) with a **circular flat mute** `ToggleButton` (icon only, tooltips *Mute* / *Unmute*) and a horizontal `Scale` for `0…volume-max`, then a **scrollable** track list **only** if the file has at least one **audio** stream (otherwise that block is **hidden**). The transport poll updates the header icon. `EventControllerScroll` on the `GLArea` changes volume when the recent grid is not covering the view. The SQLite `settings` table stores `master_volume` and `master_mute` strings, applied to mpv after the render context is created and written again on quit (before `commit_quit`).

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Volume, mute, and persistence
  Scenario: Popover and shortcuts agree with mpv
    Given the Sound popover is open or keys adjust volume
    When mute toggles or scale moves within volume-max
    Then mpv volume and mute mirror chrome without orphan notifications

  Scenario: Scroll wheel adjusts volume on video only when visible
    Given playback chrome allows wheel delivery on GLArea and grid is hidden
    When the user scrolls vertically with proportional steps
    Then volume moves by clamped increments without exceeding bounds

  Scenario: Persist last audible settings across quit
    Given playback ran long enough to reflect real volume and mute
    When the application reaches quit before commit_quit stops playback
    Then SQLite stores master_volume and master_mute for next launch defaults
```

- `volume` and `mute` on libmpv stay consistent with the popover, scroll wheel, and keys.
- The scale does not show a numeric value label (`draw_value` = false); level is read from the slider and the header icon. Mute is the icon-only toggle in the popover; mute state does not change the stored volume value (unmute restores the prior level).
- **Scroll (video):** vertical scroll on `GLArea` adjusts volume by 5% per notched step (smoothed trackpads aggregate sensibly; small `dy` values change volume proportionally, clamped to range).
- **Keys (main window, player ready):** **↑** / **↓** nudge volume by 5% (clamped); **m** toggles mute. Do not add extra notifications for volume changes.
- On startup, if settings exist, apply `volume` and `mute` to mpv after `MpvBundle` creation. Defaults: 100% volume, not muted.
- On quit, persist current `volume` and `mute` to the DB before `commit_quit` runs `stop` (so values reflect real playback, not a forced idle state from quit).

**Current code:** `src/app/build_window.rs` and `src/app/realize.rs` (header control and transport sync), `src/app/input.rs` (GL scroll and key bindings), `src/db.rs` (`settings` + load/save), `src/mpv_embed.rs` (no extra hooks beyond existing property API).
