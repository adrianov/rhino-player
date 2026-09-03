# Bob deinterlace (60 fps fields)

---
status: done
priority: p1
layers: [playback, os-integration]
related: [06, 10, 26]
scope: platform-specific
---

## Use cases
- Watch interlaced Blu-ray content (1080i / 60i) with full temporal resolution instead of combed 30 fps presentation.
- Watch local HD interlaced files (1080i) the same way, using field Bob instead of the optional Smooth motion script.
- Combine Bob deinterlace with optional Smooth motion on Blu-ray when both apply to the same title.

## Description
Targets **macOS** and **Linux**. When decoded frames are interlaced, Rhino doubles each field into a full frame (~60 Hz presentation) and leaves progressive frames alone. Hardware decode uses a path that lets the deinterlace filter read pixels.

**Blu-ray** titles keep Bob ready whenever a disc is open (stream metadata may arrive late). **Local** HD files (~1080 tall) attach Bob only after the engine reports interlaced frames; for those opens the optional Smooth motion graph is skipped so presentation is field Bob alone. **DVD** chapter files use a separate cadence path (see [26-sixty-fps-motion](26-sixty-fps-motion.md)).

## Behavior

```gherkin
@status:done @priority:p1 @layer:playback @area:bob-deinterlace
Feature: Bob deinterlace (60 fps fields)

  Scenario: Interlaced Blu-ray attaches Bob deinterlace
    Given a Blu-ray title is open for playback
    And the decoded video is interlaced
    When the playback engine applies the main video filter chain
    Then a Bob deinterlace filter is active for interlaced frames only
    And the presentation rate doubles fields to approximately 60 frames per second

  Scenario: Progressive Blu-ray does not bob progressive frames
    Given a Blu-ray title is open for playback
    And the decoded video is progressive
    When the playback engine applies the main video filter chain
    Then Bob deinterlace does not alter progressive frames

  Scenario: Local 1080i file attaches Bob without Smooth script
    Given a local HD interlaced file is open
    And the decoded frames are marked interlaced
    When the playback engine applies the main video filter chain
    Then a Bob deinterlace filter is active for interlaced frames only
    And the presentation rate doubles fields to approximately 60 frames per second
    And the optional temporal-smoothing script is not attached for this open

  Scenario: Local progressive file is unchanged
    Given a local progressive file is open
    When the playback engine applies the main video filter chain
    Then Bob deinterlace is not attached

  Scenario: Smooth motion stacks after deinterlace on Blu-ray when both apply
    Given the smooth-motion preference is on at approximately 1.0× playback speed
    And an interlaced Blu-ray title is open
    When the temporal-smoothing filter graph is applied
    Then Bob deinterlace is ordered before the optional smoothing graph
```

## Notes
- Implementation: `src/video_pref/bob_deinterlace.rs` owns Bob orchestration — `bob_prepare_apply` / `bob_finish_apply` from `apply_mpv_video`; `sync_bluray_deinterlace_mpv` (alias of `sync_bob_deinterlace_mpv`) after `clear_vf` / interleaved; `bob_blocks_smooth_vf`, `bob_vf_matches_want`, `bob_needs_apply_when_smooth_off`. Binding: mpv video filter `@rhino-deint:bwdif=mode=1:deint=interlaced` (libavfilter interlaced-only gate; **not** `cond=` — mpv 0.41 rejects that with COMMAND / `Raw(-12)`). Hardware decode uses a **-copy** path (`ensure_hwdec_vf_copy`: macOS `videotoolbox-copy`, Linux `auto-copy` / `vaapi-copy` / `nvdec-copy`, fallback `no`). Local detection: mpv **`video-frame-info/interlaced`** plus decode height in the ~1080 band (`video-params/h` / `height`); sticky path for the open because Bob output looks progressive to the engine. Blu-ray detection: `bd://` / `bluray://` or shell path via `is_bluray_disc_path` (`me_budget_shell_path` on `try_load`).
