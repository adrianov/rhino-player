# DVD unified timeline (all chapter VOBs)

---
status: done
priority: p1
layers: [ui, playback, storage]
related: [04, 06, 07, 09, 18, 31]
scope: portable
---

## Use cases
- Scrub and read elapsed/total time across an entire DVD title, not only the current chapter file.
- See where each chapter starts on one seek bar.
- Jump to any chapter by scrubbing, not only with Previous / Next.

## Description
When playback uses chapter files under a **video transport folder** (sorted `.vob` files in the same tree), the app treats them as **one title**: one total duration, one playback position on the transport bar, and marks at chapter boundaries. Automatic advance at the end of a chapter still loads the next chapter file in the same order as today. Opening the disc folder or any chapter in that tree enters this mode.

Per-chapter files remain how the playback engine loads media (no requirement for a single combined file on disk).

## Behavior

```gherkin
@status:done @priority:p1 @layer:playback @area:dvd-timeline
Feature: DVD unified timeline

  Scenario: Seek bar spans all chapters in the transport folder
    Given a sorted list of chapter files is active under one video transport folder
    And a total title duration is known from stored chapter lengths
    When the user views the transport bar during playback
    Then the bar maximum reflects the sum of chapter durations
    And the thumb position reflects playback time within the whole title

  Scenario: Scrubbing jumps to the correct chapter and offset
    Given multiple chapters exist in the transport folder
    And playback was running before the scrub
    When the user scrubs to a time inside another chapter file in that title
    Then the matching chapter file loads if it is not already open
    And playback resumes at the offset within that chapter
    And playback continues without the user pressing play again

  Scenario: Chapter boundaries appear on the seek bar
    Given more than one chapter is in the active list
    When the seek bar is shown
    Then a mark appears at the start of each chapter after the first

  Scenario: Chapter EOF advances within the same title
    Given unified timeline mode is active for one title set
    And playback was running when the current chapter ends
    When a chapter ends before the whole title ends
    Then the next chapter file in that same title set loads automatically
    And playback continues without the user pressing play again
    And chapters from a different title on the same disc do not load until the title is finished

  Scenario: Prev and Next jump sibling disc folders
    Given unified timeline mode is active for one title set
    And sibling disc directories each contain a video transport folder under the same parent
    When the user activates Previous or Next on the bottom bar
    Then the target is the resume-aware first chapter of the previous or next sibling disc directory
    And not the previous or next chapter file within the current disc

  Scenario: Single-chapter or unknown lengths fall back to one file
    Given only one chapter file is in the list
    Or total duration cannot be estimated yet
    When the user views the transport bar
    Then behavior matches a normal single-file title until more chapter lengths are known
```

## Notes
- Detection: `video_ext::is_dvd_vob_path`; chapter list = same-title `.vob` files in one `VIDEO_TS/` (`VTS_02_1` groups with `VTS_02_2`, not `VTS_01_*` or `VTS_03_*`), via `dvd_entity::video_ts_for_vob` + `list_title_vobs`, natural sort. Mid-title chapter EOF uses that list via `advance_title_chapter_eof` (not every `.vob` in the folder). Bottom-bar **Previous** / **Next** and title-end sibling advance use sibling **disc directories** under the same parent ([07](07-sibling-folder-queue.md)), opening `dvd_first_playable_vob` on the neighbour disc. Folder/disc open: `video_ext::dvd_first_playable_vob` — `pick_main_dvd_vob` (most chapters; ties → lowest `VTS_XX`, skip `VTS_01` when `VTS_02+` exist) then SQLite resume on that **entity** key only (not other titles on the disc). Title EOF uses global bar position; mid-title chapter EOF loads the next chapter in the same title via `advance_title_chapter_eof`.
- `resolve_global` uses per-chapter duration windows (not start-only), so scrubbing maps to the correct file when SQLite has chapter lengths.
- Seek preview: `preview_hover_duration` must not cap the bar to the open chapter’s mpv `duration` while unified mode is active.
- **Persistence:** multi-part DVD titles use one [playback entity](31-playback-entity.md) row (disc folder containing `VIDEO_TS/`); `time_pos_sec` and `duration_sec` are **whole-title** seconds. Per-chapter `.vob` rows are purged on write. Resume on load maps global time → chapter file + local offset (`playback_entity` → `dvd_entity::resume_chapter_and_local`).
- Chapter lengths for the bar: stored **entity** `duration_sec` (whole title) always wins over the open chapter’s mpv `duration` when building the timeline; per-chapter rows in SQLite, Rust IFO parse (`dvd_ifo_parse`), in-session `grow_chapter_dur`, or `.vob` byte-size bootstrap fill gaps. Bar cache (`DvdBarState`) rebuilt on `FileLoaded` with live mpv duration. Scrub uses `dvd_hold_global` until resume applies.
- **IFO files on disk** (`dvd_ifo_parse`): `VIDEO_TS.IFO` **TT_SRPT** picks the main feature title (`pick_main` / `dvd_first_playable_vob`); `VTS_XX_0.IFO` **PGC** cell `playback_time` + **PTT** build the unified bar (`from_chapter_ifo`) even when only some `.vob` files are on disk. When the IFO lists fewer VOB ids than chapter files present (`VTS_02_1` … `VTS_02_N`), `expand_on_disk_chapters` maps the title duration across on-disk files by byte size so EOF advance and the seek bar include every rip segment. PTT marks on the seek bar. Without IFO: SQLite + `.vob` byte bootstrap. Cross-chapter scrub uses the cached bar timeline (`seek_global` + shared `dvd_bar` slot); `load_chapter_seek` stashes chapter-local seconds in `pending_resume`; `apply_pending_resume` waits until mpv `path` matches the shell path and `duration` is known (`FileLoaded`, `path`, `duration`). Mid-title EOF: `advance_title_chapter_eof` + `eof-reached` / local tail; sibling fallback uses chapter-local tail when the unified bar is not at title end. Debug: `RHINO_DVD_SEEK=1`.
- Mid-title chapter EOF: `load_chapter_seek(..., resume_playing=true, chapter_eof=true)` unpause after resume seek even when mpv is paused at `eof-reached`.
- Cross-chapter scrub: `seek_global` passes scrub-time **`resume_playing`** (from transport, not mpv pause at EOF); seek-bar release uses the thumb value, not stale hover time.
- Implementation owner: `src/dvd_vob_timeline.rs` — map global time ↔ `(chapter path, local time)`; transport + seek wiring.
- Seek bar preview ([18](18-thumbnail-preview.md)): uses the chapter file for the scrub target once mapped; hover labels use `chapter_preview_labels` (includes **Chapter 1** at title start when there are two or more chapters — hidden for single-chapter titles).
- Distinct from `dvd://` / libdvdread (optional future if the engine exposes one native timeline).
