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

  Scenario: Chapter EOF advances across feature title sets on one disc
    Given unified timeline mode is active for one disc
    And the disc has more than one feature title set in the transport queue
    And playback was running when the current chapter ends
    When a chapter ends before the whole disc feature ends
    Then the next chapter file in the disc feature queue loads automatically
    And playback continues without the user pressing play again

  Scenario: Chapter EOF does not load menu-only title sets
    Given unified timeline mode is active for one disc
    And a menu-only title set exists alongside feature sets
    When playback reaches the end of the last feature chapter
    Then no menu-only chapter file loads automatically

  Scenario: Prev and Next jump sibling disc folders
    Given unified timeline mode is active for one title set
    And sibling disc directories each contain a video transport folder under the same parent
    When the user activates Previous or Next on the bottom bar
    Then the target is the resume-aware first chapter of the previous or next sibling disc directory
    And not the previous or next chapter file within the current disc

  Scenario: Fresh disc open skips interactive menu preamble
    Given a disc folder is opened for the first time with no stored resume
    And the disc main feature title set includes a shorter menu title track before the movie track
    When playback starts
    Then the movie begins after the menu preamble
    And the transport bar reflects the movie position on the unified timeline

  Scenario: Single-chapter or unknown lengths fall back to one file
    Given only one chapter file is in the list
    Or total duration cannot be estimated yet
    When the user views the transport bar
    Then behavior matches a normal single-file title until more chapter lengths are known
```

## Notes
- Detection: `video_ext::is_dvd_vob_path`; unified timeline queue = **consecutive substantial** feature title sets on the disc (IFO title length ≥ ~5 min per `VTS_xx`, contiguous block containing the open chapter) via `dvd_entity::timeline_chapter_paths`. Per-title-set lists remain in `title_chapter_paths` (Sound/Sub IFO, chain-head probe). Mid-title EOF uses `advance_title_chapter_eof` across that queue (e.g. `VTS_02_4` → `VTS_03_1` on two-part discs). `list_feature_vobs` lists every substantial set on the disc for tests. Bottom-bar **Previous** / **Next** and title-end sibling advance use sibling **disc directories** under the same parent ([07](07-sibling-folder-queue.md)), opening `dvd_first_playable_vob` on the neighbour disc. Folder/disc open: `pick_main_dvd_vob` on the largest on-disk title set; **`main_title_from_disc`** picks the longest **TTN** playback within that set (movie, not menu TTN). **Fresh open** with no SQLite resume: `movie_entry_global_sec` skips prior TTN preamble (`load_file_path` pending resume). Stored resume on the **entity** key when present.
- `resolve_global` uses per-`.vob` duration windows from the in-session bar cache (not start-only), so scrubbing maps to the correct file when segment lengths are known.
- Seek preview: `preview_hover_duration` must not cap the bar to the open chapter’s mpv `duration` while unified mode is active.
- **Persistence:** one [playback entity](31-playback-entity.md) row per disc (folder containing `VIDEO_TS/`): `time_pos_sec` and `duration_sec` are **whole-title** seconds only. No per-`.vob` SQLite rows — legacy chapter rows are deleted on entity write. Resume on load maps global time → `.vob` file + local offset (`playback_entity` → `dvd_entity::resume_chapter_and_local`).
- Segment lengths for the bar: **`VTS_xx_0.IFO` PGC cell playback times** mapped onto each on-disk `.vob` by **cell `first_sector` / `last_sector`** within each title set (`dvd_ifo_parse::title_vob_sector_durs`; one IFO per `VTS_xx`, concatenated for multi-set discs). IFO-identified menu stubs (`VTS_xx_1` ≈1 s) are **dropped only when the on-disk file is tiny** (&lt;100 MiB). mpv live/probe durations **do not grow or shrink** IFO segments afterward. **`first_substantial_vob`** picks the folder-open chapter within one title set. Without IFO, in-session `DvdBarState` cache (`merge_prior_durs`), clamped mpv live `duration`, then libmpv probe fill gaps (chain-head `.vob` probe uses sibling file size when demuxer reports the whole program). **`FileLoaded` bar build** (`TimelineBuildOpts::PLAYBACK`) is cache-only for mpv probe. Total = sum of sector-mapped per-file IFO durations across all feature sets on the disc.
- **IFO files on disk** (`dvd_ifo_parse`): **main-title pick** when opening a disc folder; **PGC cell durations** for the unified timeline and seek/`resolve_global`; **PTT chapter marks** for seek-bar / preview labels (`chapter_labels_for_timeline` scales each block onto measured `.vob` totals). Audio/sub stream attrs from `VTS_xx_0.IFO`.
- Mid-title chapter EOF: `load_chapter_seek(..., resume_playing=true, chapter_eof=true)` unpause after resume seek even when mpv is paused at `eof-reached`. EOF advance maps **live** mpv tail `time-pos` / `duration` through `continue_after_vob_eof` (not `next_chapter_after` / next-file timeline start) so a longer open `.vob` than SQLite still seeks into the correct offset of the next on-disk `.vob`. **Transport tick** calls `advance_title_chapter_eof` every second (not only when `core-idle` / `eof-reached`) because `keep-open=yes` and Smooth `vf` often leave mpv non-idle at a `.vob` tail; `chapter_local_at_eof` gates the advance inside that path. `refresh_dvd_bar_at_chapter_eof` rebuilds when live mpv `duration` is shorter than the cached segment length.
- Cross-chapter scrub: `seek_global` passes scrub-time **`resume_playing`** (from transport, not mpv pause at EOF); seek-bar release uses GtkRange thumb position capped like the preview label (`seek_bar_label_time_from_value`).
- Implementation owner: `src/dvd_vob_timeline.rs` — map global time ↔ `(vob path, local time)`; transport + seek wiring.
- Seek bar preview ([18](18-thumbnail-preview.md)): scrub target file comes from VOB `resolve_global`; hover labels and preview seek cap use `DvdBarState::chapter_preview_labels` + `preview_chapter_dur` (IFO **Chapter N** marks per `VTS_xx` block, scaled to VOB totals — empty when the title has one PTT chapter or no IFO).
- Distinct from `dvd://` / libdvdread (optional future if the engine exposes one native timeline).
