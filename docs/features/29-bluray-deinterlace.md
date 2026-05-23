# Blu-ray Bob deinterlace (60 fps fields)

---
status: done
priority: p1
layers: [playback, os-integration]
related: [06, 10, 26]
scope: platform-specific
---

## Use cases
- Watch interlaced Blu-ray content (1080i / 60i) with full temporal resolution instead of combed 30 fps presentation.
- Combine Bob deinterlace with optional Smooth motion when both apply to the same title.

## Description
Targets **macOS** and **Linux** ports using **mpv** as the playback engine. When the open item is a **Blu-ray** disc, Rhino attaches an mpv video filter that runs **BWDIF** in **Bob** mode (**mode=1**) only when mpv marks frames **interlaced**, turning each field into a full frame (~60 Hz). Progressive chapters are untouched via the filter’s per-frame condition. Hardware decode switches to a **copy** path so filters can process pixels. **DVD** chapter files use a separate cadence path (see [26-sixty-fps-motion](26-sixty-fps-motion.md)); this feature is **Blu-ray-only**.

## Behavior

```gherkin
@status:done @priority:p1 @layer:playback @area:bluray-deinterlace
Feature: Blu-ray Bob deinterlace

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

  Scenario: Non-Blu-ray local files are unchanged
    Given a local progressive file is open
    When the playback engine applies the main video filter chain
    Then Bob deinterlace for Blu-ray is not attached

  Scenario: Smooth motion stacks after deinterlace when both apply
    Given the smooth-motion preference is on at approximately 1.0× playback speed
    And an interlaced Blu-ray title is open
    When the temporal-smoothing filter graph is applied
    Then Bob deinterlace is ordered before the optional smoothing graph
```

## Notes
- Implementation: `src/video_pref/bluray_deinterlace.rs` — `bluray_playback_active`, `wants_bluray_bob_deinterlace` (Blu-ray open only), `attach_bluray_deinterlace`, `sync_bluray_deinterlace_mpv`, `ensure_hwdec_vf_copy`. mpv `vf add` uses `@rhino-deint:bwdif=mode=1:deint=interlaced` (libavfilter interlaced-only gate; **not** `cond=` in the vf string — mpv 0.41 rejects that with COMMAND / `Raw(-12)`). **hwdec** candidates: macOS `videotoolbox-copy`, Linux `auto-copy` / `vaapi-copy` / `nvdec-copy`, fallback `no`. `sync_bluray_deinterlace_mpv` runs at the start of `apply_mpv_video_impl` when media is open (including Smooth **display-resample** path) and after `clear_vf`. `vf_smooth_matches_prefs` requires the deinterlace label when Blu-ray is active. Blu-ray detection: `bd://` / `bluray://` or shell path via `is_bluray_disc_path` (`me_budget_shell_path` on `try_load`).
