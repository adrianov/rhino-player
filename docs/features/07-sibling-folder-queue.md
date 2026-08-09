# Sibling folder queue (folder playback)

---
status: done
priority: p1
layers: [playback, storage, ui]
related: [02, 03, 04, 06, 13, 21, 28]
---

## Use cases
- After a file ends, continue with the next file in the same directory.
- When the directory is exhausted, continue in the next sibling subdirectory under the same parent (e.g. next season folder).
- Browse a folder tree without going back to **Open** between files.
- Keep watching an unfinished download without jumping to the next folder neighbour when playable data runs out.

## Description
On natural end of playback, the next file in **sorted** order in the current directory loads automatically. If the current file was the last in its directory, the app looks only among **sibling directories that share the same immediate enclosing directory** (e.g. next season folder beside the current season); the first sorted video in the next such directory loads, and empty siblings are skipped. The queue **does not** walk further up the tree to other directory groups (e.g. it does not jump from one show folder to an unrelated show folder that lives beside it under a shared library folder). With no next file in-folder and no later sibling directory with a video, playback stops (no wrap).

Sibling videos use the shared extension list from open / folder scan; the listing is non-recursive per directory and ordering uses natural lexical comparison (case-insensitive, with natural digit runs). Bottom-bar **Previous** / **Next** use the same ordering when a local file with duration is loaded.

An unfinished Direct Connect download (path still using the incomplete-download suffix) is special: when playback reaches natural end, the app treats that as “more of the file is not on disk yet”, pauses on the same file, and does not auto-advance. Unpausing tries to resume from that point once more bytes are available. Demux duration alone is not used as “finished” while the incomplete suffix remains.

## Behavior

```gherkin
@status:done @priority:p1 @layer:playback @area:sibling-queue
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
    Given there is no next sorted file in the current directory
    And no later sibling directory under the same immediate enclosing directory holds a next playable file
    When end-of-playback occurs
    Then playback stops without wrapping back to the first folder

  Scenario: Queue does not leave the sibling group for another folder beside it
    Given the current file is last in its directory and playable files exist only under a different directory that is a sibling of the directory that contains the current directory
    When end-of-playback occurs
    Then playback stops without loading those files

  Scenario: Exit After Current Video overrides advance
    Given the session-only Exit After Current Video option is enabled
    When end-of-playback occurs
    Then the application exits without loading another sibling file

  Scenario: Auto-advance when container duration exceeds decoded streams
    Given the loaded file is not an unfinished download
    And the loaded file reports a longer container duration than the longest decoded stream
    And playback stalls before the reported duration ends
    When natural end-of-playback occurs
    Then the next sorted sibling file loads automatically

  Scenario: Unfinished download end pauses instead of advancing
    Given the loaded file is an unfinished download still named with the incomplete-download suffix
    When natural end-of-playback occurs
    Then playback pauses on the same file
    And no sibling file loads automatically

  Scenario: Unpause after unfinished download pause resumes the same file
    Given playback paused on an unfinished download after natural end
    When the user unpauses
    Then the player tries to resume the same file from that position
    And no sibling file loads automatically while the path still uses the incomplete-download suffix

  Scenario: Manual Prev / Next matches automatic order
    Given a local file with duration is open
    When the user activates the bottom-bar Previous or Next
    Then the loaded file matches the same folder and sibling ordering as EOF advance

  Scenario: DVD chapter files queue sibling disc directories
    Given playback is a chapter file under a video transport folder on a disc directory
    And another disc directory with a video transport folder is a sibling under the same parent
    When the user activates Next or end-of-playback occurs after the whole title finishes
    Then the next sibling disc directory opens at its resume-aware entry chapter
    And not the next chapter file within the current disc unless chapter EOF advance applies

  Scenario: Ctrl with arrows jumps previous / next sibling
    Given a local file with duration is open
    When the user presses Ctrl+Left or Ctrl+Right (including keypad arrows with Ctrl)
    Then the loaded file matches the same outcome as activating Previous or Next respectively

  Scenario: Tooltips reflect the next / previous filename
    Given a sibling target exists for Previous or Next
    When the user hovers the corresponding button
    Then the tooltip shows the target filename
    And buttons without a sibling target show a no-neighbour tooltip

  Scenario: Sibling tooltips keep non-Latin titles
    Given nearby videos use non-Latin letters in their filenames
    When the bottom-bar sibling controls update after a file loads
    Then Previous and Next tooltips show those neighbour titles
    And the viewer stays open
```

## Notes
- When container duration exceeds the decoded tail (common on MKV with mismatched stream lengths), sibling advance uses a wider tail window (`NEAR_END_SEC`) while `core-idle`; transport clamps the seek bar to `time-pos` within that window after playback has entered the tail. Auto-advance requires **playing into** the tail (≥1s of position movement since load), not opening with resume already near end — avoids a chain load on continue-grid open.
- Unfinished Direct Connect paths (`*.dctmp`): `IncompleteEofHold` pauses on every natural EOF and never auto-advances while the suffix remains (demux `duration` often equals the downloaded prefix). Play/Pause unpause arms continue **before** clearing `pause`, then re-seeks absolute+exact so demux can pick up newly grown bytes.
- Sibling tooltip labels use `human_media_title` → `peel_download_temp`; strip the incomplete suffix with `str::get` (UTF-8–safe). A trailing byte slice used to panic on some non-Latin names plus odd-length extensions (e.g. `.webm`).
- Before loading the next sibling after EOF, mpv `speed` is set to **1.0** when it was not already (see [28-playback-speed](28-playback-speed.md)).
- The last successfully loaded canonical path is used when `path` is empty.
- Local files only: with no resolvable path, no advance runs.
- Implementation: `src/sibling_advance.rs`, `src/incomplete_download_eof.rs` (`IncompleteEofHold`), `src/app/load.rs::maybe_advance_sibling_on_eof`, `src/app/transport_events.rs`.
- Sensitivity and tooltips update on `path` change plus `FileLoaded` / `VideoReconfig`; chrome controls update from `mpv_observe_property` events with no-op-change guards (so tooltip-show timers are not reset).
- Out of scope here: arbitrary multi-title playlists, shuffle, MIME probing.
