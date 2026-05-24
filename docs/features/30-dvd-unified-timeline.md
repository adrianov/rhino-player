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
- Detection: `video_ext::is_dvd_vob_path`; feature queue = every chapter `.vob` on the disc (`VTS_02_1` … `VTS_03_N`, …) via `dvd_entity::list_feature_vobs` — all non-menu title sets (`VTS_01` skipped when `VTS_02+` exist), natural sort. Mid-title EOF uses `advance_title_chapter_eof` across that full queue (e.g. `VTS_02_4` → `VTS_03_1` on the same disc before sibling-folder advance). Bottom-bar **Previous** / **Next** and title-end sibling advance use sibling **disc directories** under the same parent ([07](07-sibling-folder-queue.md)), opening `dvd_first_playable_vob` on the neighbour disc. Folder/disc open: `video_ext::dvd_first_playable_vob` — `pick_main_dvd_vob` (most chapters; ties → lowest `VTS_XX`, skip `VTS_01` when `VTS_02+` exist) then SQLite resume on that **entity** key only (not other titles on the disc). Title EOF uses global bar position; mid-title chapter EOF loads the next `.vob` in the feature queue via `advance_title_chapter_eof`.
- `resolve_global` uses per-`.vob` duration windows from the in-session bar cache (not start-only), so scrubbing maps to the correct file when segment lengths are known.
- Seek preview: `preview_hover_duration` must not cap the bar to the open chapter’s mpv `duration` while unified mode is active.
- **Persistence:** one [playback entity](31-playback-entity.md) row per disc (folder containing `VIDEO_TS/`): `time_pos_sec` and `duration_sec` are **whole-title** seconds only. No per-`.vob` SQLite rows — legacy chapter rows are deleted on entity write. Resume on load maps global time → `.vob` file + local offset (`playback_entity` → `dvd_entity::resume_chapter_and_local`).
- Segment lengths for the bar: in-session `DvdBarState` cache (`merge_prior_durs`), mpv live `duration` on the open chapter, or byte-size fill **only when mpv or another segment provides an anchor** (no fixed bytes/sec guess). Total = sum of queued `.vob` durations. Bar cache keeps the prior timeline when a `FileLoaded` rebuild would inflate total before mpv `duration` is ready.
- **IFO files on disk** (`dvd_ifo_parse`): **main-title pick** when opening a disc folder; **PTT chapter marks** for seek-bar / preview labels only — one `VTS_xx_0.IFO` per title set in the feature queue (`chapter_labels_for_timeline` scales each block onto measured `.vob` totals; not used for `resolve_global` or cross-`.vob` load). Audio/sub stream attrs from `VTS_xx_0.IFO`.
- Mid-title chapter EOF: `load_chapter_seek(..., resume_playing=true, chapter_eof=true)` unpause after resume seek even when mpv is paused at `eof-reached`. EOF advance maps **live** mpv tail `time-pos` / `duration` through `continue_after_vob_eof` (not `next_chapter_after` / next-file timeline start) so a longer open `.vob` than SQLite still seeks into the correct offset of the next on-disk `.vob`. **Transport tick** calls `advance_title_chapter_eof` every second (not only when `core-idle` / `eof-reached`) because `keep-open=yes` and Smooth `vf` often leave mpv non-idle at a `.vob` tail; `chapter_local_at_eof` gates the advance inside that path. `refresh_dvd_bar_at_chapter_eof` rebuilds when live mpv `duration` is shorter than the cached segment length.
- Cross-chapter scrub: `seek_global` passes scrub-time **`resume_playing`** (from transport, not mpv pause at EOF); seek-bar release uses GtkRange thumb position capped like the preview label (`seek_bar_label_time_from_value`).
- Implementation owner: `src/dvd_vob_timeline.rs` — map global time ↔ `(vob path, local time)`; transport + seek wiring.
- Seek bar preview ([18](18-thumbnail-preview.md)): scrub target file comes from VOB `resolve_global`; hover labels and preview seek cap use `DvdBarState::chapter_preview_labels` + `preview_chapter_dur` (IFO **Chapter N** marks per `VTS_xx` block, scaled to VOB totals — empty when the title has one PTT chapter or no IFO).
- Distinct from `dvd://` / libdvdread (optional future if the engine exposes one native timeline).
