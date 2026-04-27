# Playback speed (1.0x / 1.5x / 2.0x)

**Name:** Fixed-step playback speed control

**Implementation status:** Done (header `speedometer-symbolic` popover + list; libmpv `speed`)

**Use cases:** Watch lecture or long scenes faster, or return to 1.0 for normal motion.

**Short description:** **1.0×**, **1.5×**, and **2.0×** are selected by setting libmpv’s `speed`. A **header** `MenuButton` (**speedometer-symbolic**) opens a popover with a `ListBox` of those three labels. No free-form slider in v1.

**Long description:** Speed applies to the current mpv play session and follows **Next** / **Previous** the same as other transport state: libmpv usually keeps the last `speed` when loading a new file in the same process. The UI re-syncs from `speed` when a file is loaded. If `speed` is not exactly one of the three, the app snaps to the **nearest** and writes that value to mpv so the list always matches. Choosing a step updates mpv, syncs the list, and **closes the popover**.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Fixed-step playback speed
  Scenario: Canonical speeds only in UI and mpv
    Given media with measurable duration is loaded
    When the user selects 1.0×, 1.5×, or 2.0× from the header list
    Then mpv speed equals that canonical step and list highlight stays synchronized via guard flags

  Scenario: Snap drift after external speed changes
    Given mpv reports speed outside the canonical trio beyond tolerance after load
    When sync logic runs after FileLoaded hooks
    Then speed snaps to nearest canonical step so rows never diverge silently from mpv

  Scenario: Disabled without playable timeline
    Given duration is unavailable so transport disables seeking
    When the speed button sensitivity mirrors seek eligibility rules
    Then speed controls remain inactive until duration becomes usable like seek bar policy

  Scenario: Interaction with Smooth Video vf layer
    Given sixty-fps-motion vf depends on approximate 1.0× playback
    When user chooses non-1.0 speeds from this control
    Then vapoursynth insertion follows sixty-fps-motion document rather than silent mismatch
```

- **mpv:** `speed` in `{ 1.0, 1.5, 2.0 }` only. Read `speed` after load; if not within 0.01 of one, pick **nearest** and `set` to that canonical value.
- **UI — header (LTR, left of subtitles/volume/hamburger end cluster):** `gtk::MenuButton` (icon: `speedometer-symbolic` where the theme provides it) + `gtk::Popover` with heading “Playback speed” and a `ListBox` of `1.0×`, `1.5×`, `2.0×`. **Disabled** when there is no open media; **enabled** when `duration` is available like the seek bar.
- **Events:** On row selection, `set` `speed` and `queue_render` the GL area as needed. Use a re-entrancy flag so programmatic `select_row` (sync from mpv) does not loop. Auto-hide and menu switching treat the speed popover like volume/subtitles.
- **No** SQLite or preferences persistence in v1 (in-memory; mpv may keep speed across `loadfile` in one run).
- **Acceptance (manual):** With a local file, pick 1.5× and 2.0× from the header—audio and video stay in sync; pick 1.0× to restore. Next file in folder keeps prior speed unless a fresh open resets behavior per mpv; UI matches `speed` after each open.

**See also:** [04-transport-and-progress](04-transport-and-progress.md), [26-sixty-fps-motion](26-sixty-fps-motion.md) (VapourSynth `RHINO_PLAYBACK_SPEED` matches this control), [10-video-options](10-video-options.md) (wider VDF menu when implemented).
