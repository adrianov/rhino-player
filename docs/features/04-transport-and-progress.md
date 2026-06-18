# Transport: play, pause, seek, progress UI

---
status: done
priority: p0
layers: [ui, mpv]
related: [03, 07, 13, 18, 22, 28]
mpv_props: [time-pos, duration, pause, mute, volume, volume-max, speed, fullscreen, media-title]
settings: [show-remaining]
---

## Use cases
- Control playback without leaving the app.
- See current position and total length in human-friendly form.
- Enter fullscreen for focused viewing.

## Description
The bottom bar (LTR) has Previous, Next, Play / Pause, elapsed time, the seek bar, total time, and a trailing **Close Video**. The header carries Speed (left), Subtitles, Volume / Sound, and the main menu (right). Times format human-friendly, including days / hours when needed. Seeks use mpv `seek <seconds> absolute+keyframes` to keep audio and video aligned, including with active video filters.

Previous and Next follow [07-sibling-folder-queue](07-sibling-folder-queue.md). Speed lives in the header per [28-playback-speed](28-playback-speed.md). Volume / Mute lives in the Sound popover per [22-audio-volume-mute](22-audio-volume-mute.md).

## Behavior

```gherkin
@status:done @priority:p0 @layer:ui @area:transport
Feature: Transport controls and progress

  Scenario: Seek reflects the user position
    Given a file with known duration is playing or paused
    When the user moves the seek bar to a new position
    Then playback jumps to that position via seek absolute+keyframes
    And audio and video remain aligned

  Scenario: Seek bar is disabled without measurable duration
    Given duration is unknown or zero
    When the user inspects the transport bar
    Then the seek control is disabled
    And the speed control sensitivity matches the seek bar

  Scenario: Long single-file duration is not capped like a DVD chapter
    Given a local file longer than four hours is open with a known duration from the playback engine
    When the transport bar updates
    Then the total time label shows the full file length
    And the seek bar range matches that length

  Scenario: Play / Pause is enabled only when duration is known
    Given a media file is loading
    When duration becomes greater than zero
    Then Play / Pause becomes sensitive and reflects the pause property
    And toggling Play / Pause flips the pause property like Space

  Scenario: Previous and Next reflect sibling targets
    Given a local file is open with siblings in the folder
    When mpv path changes or FileLoaded fires
    Then Previous and Next sensitivity reflects the existence of a sibling target
    And tooltips show the next / previous filename or a no-neighbour string

  Scenario: Time label respects show-remaining
    Given show-remaining is true
    When time-pos updates
    Then the secondary label shows negative remaining time
    And switching show-remaining flips back to elapsed

  Scenario: Paused seek with active VapourSynth vf
    Given playback is paused and the active vf contains vapoursynth
    When the user seeks
    Then the vf is temporarily cleared so mpv renders a normal still frame
    And Smooth 60 reapplies on the next unpause if the preference remains enabled
```

## Notes
- Properties observed: `time-pos`, `duration`, `pause`, `mute`, `volume`, `volume-max`, `speed`, `path`, `fullscreen`, `media-title`.
- When `core-idle` or `eof-reached` and position is within `NEAR_END_SEC` of reported duration but streams ended earlier, the seek bar and duration label use `time-pos` after playback has entered the tail (same gate as sibling EOF advance).
- Per-chapter DVD `.vob` duration cap (`MAX_VOB_DUR_SEC`, 4 h) applies only to unified DVD title timelines — not to single-file media (long MKV/MP4 must keep mpv `duration` intact).
- Seek uses `seek <seconds> absolute+keyframes` (fallback: setting `time-pos`).
- Optional hover preview popover is owned by [18-thumbnail-preview](18-thumbnail-preview.md).
- Keyboard arrow seeks (± five seconds) live in this feature; transport uses the same seek behavior as the bottom bar ([04-transport-and-progress](04-transport-and-progress.md)).
