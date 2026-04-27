# Tracks: audio, video, subtitles

---
status: wip
priority: p1
layers: [ui, mpv, db]
related: [22, 24]
mpv_props: [track-list, aid, sid, vid]
settings: [aid_per_path, preferred_audio_track_label]
---

## Use cases
- Watch in the right language.
- Pick an alternate video stream when a file has several.
- Add an external subtitle or audio file (later).

## Description
The `track-list` property drives the UI. Today the **Sound** popover (header) hosts the audio-track list when the file has at least two `type: audio` entries; subtitle handling is owned by [24-subtitles](24-subtitles.md). Video-track switching and external `sub-add` / `audio-add` flows are planned.

For audio, choosing a row sets mpv `aid` to that track id. The choice persists per local-file path in SQLite and updates a global preferred audio-track label. After each load, the app first restores the saved per-file `aid`; otherwise it picks the closest Levenshtein match to the global label, repairs `aid=no` when several tracks exist, and sets `aid` to the only id when exactly one exists.

## Behavior

```gherkin
@status:wip @priority:p1 @layer:mpv @area:tracks
Feature: Audio track selection

  Scenario: Sound popover shows audio list when multiple streams exist
    Given the current track-list contains at least two audio streams
    When the user opens the Sound control
    Then a scrollable radio list of audio tracks appears above the Volume row
    And the row matching the current aid is selected

  Scenario: Single audio stream hides the track block
    Given the current track-list contains zero or one audio streams
    When the user opens the Sound control
    Then no track block appears
    And only the Volume row is visible

  Scenario: Selecting a track persists per-file and globally
    Given multiple audio streams exist for the loaded file
    When the user selects a different audio row
    Then mpv aid updates to that track id
    And SQLite stores the choice per local-file path
    And the global preferred audio-track label is updated to the row text

  Scenario: Restored aid on load
    Given a saved per-file aid exists and that track still exists in track-list
    When the file finishes loading and the delayed apply runs
    Then aid is restored to the saved id before global name-based preference is considered

  Scenario: Repair aid=no with multiple streams
    Given several audio streams exist and aid is no
    When the delayed apply runs
    Then aid is set to a valid track using the global preferred label

  Scenario: Single-stream file selects the only id
    Given exactly one audio stream exists and no per-file choice is stored
    When the delayed apply runs
    Then aid is set to that one track id
```

## Notes
- The per-file aid is stored before the watch-later path so it survives SIGTERM / kill.
- No "None" / no-audio row in the popover; **mute** covers that.
- Errors setting `aid` are ignored in the UI (logs only).
- Video-track switching is reserved for a later iteration; show the control only if more than one non-album-art video track exists and update on `track-list` change without requiring a popover re-open.
