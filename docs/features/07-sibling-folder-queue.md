# Sibling folder queue (folder playback)

---
status: done
priority: p1
layers: [mpv, fs, ui]
related: [02, 03, 04, 05, 06, 13, 28]
mpv_props: [eof-reached, time-pos, duration, path]
---

## Use cases
- After a file ends, continue with the next file in the same directory.
- When the directory is exhausted, continue in the next sibling subdirectory under the same parent (e.g. next season folder).
- Browse a folder tree without going back to **Open** between files.

## Description
On natural EOF, the next file in **sorted** order in the current directory loads automatically. If the current file was the last in its directory, the app walks up one level, finds the next sibling directory in sorted order, and loads the first sorted video there; empty siblings are skipped. With no further sibling at any level, playback stops (no wrap).

Sibling videos use the shared extension list from `src/video_ext.rs` (same as **Open Video**); the listing is non-recursive per directory and ordering uses `lexical_sort::natural_lexical_cmp` (case-insensitive, with natural digit runs). Bottom-bar **Previous** / **Next** use the same ordering when a local file with duration is loaded.

The primary triggers are natural end-of-playback signals from the playback engine (`eof-reached` and end-of-file with EOF reason). There is no timed tail poll.

## Behavior

```gherkin
@status:done @priority:p1 @layer:mpv @area:sibling-queue
Feature: Sibling folder queue

  Background:
    Given the loaded media is a local file with siblings discoverable per the documented extension list

  Scenario: Auto-advance to next file in folder
    Given the current file has a next sibling in sorted order
    When natural end-of-playback occurs
    Then the next sorted video in the same directory loads automatically
    And the playback rate is the default normal speed
    And resume snapshot writes follow try_load rules

  Scenario: Advance to next sibling folder when current folder is exhausted
    Given the current file is the last in its directory by sorted order
    When end-of-playback occurs
    Then the first sorted video in the next sibling subdirectory under the parent loads
    And empty sibling subdirectories are skipped

  Scenario: No further sibling stops playback
    Given no next file or sibling folder exists at any level up to the configured walk-up limit
    When end-of-playback occurs
    Then playback stops without wrapping back to the first folder

  Scenario: Exit After Current Video overrides advance
    Given the session-only Exit After Current Video option is enabled
    When end-of-playback occurs
    Then the application exits without loading another sibling file

  Scenario: Manual Prev / Next matches automatic order
    Given a local file with duration is open
    When the user activates the bottom-bar Previous or Next
    Then the loaded file matches the same folder and sibling ordering as EOF advance

  Scenario: Tooltips reflect the next / previous filename
    Given a sibling target exists for Previous or Next
    When the user hovers the corresponding button
    Then the tooltip shows the target filename
    And buttons without a sibling target show a no-neighbour tooltip
```

## Notes
- Trigger sources: `eof-reached==true`, mpv `EndFile` with EOF reason, via the mpv event drain.
- Before loading the next sibling after EOF, mpv `speed` is set to **1.0** when it was not already (see [28-playback-speed](28-playback-speed.md)).
- The last successfully loaded canonical path is used when `path` is empty.
- Local files only: with no resolvable path, no advance runs.
- Implementation: `src/sibling_advance.rs`, `src/app/load.rs::maybe_advance_sibling_on_eof`, `src/app/transport_events.rs::wire_transport_events`.
- Sensitivity and tooltips update on `path` change plus `FileLoaded` / `VideoReconfig`; chrome controls update from `mpv_observe_property` events with no-op-change guards (so tooltip-show timers are not reset).
- Out of scope here: m3u playlist UI, shuffle, MIME probing — those belong with [05-playlist](05-playlist.md).
