# Smooth video playback (display / interpolation) — removed

---
status: removed
priority: p2
layers: [mpv]
related: [26]
---

## Use cases
- Historical: judder-free presentation on fixed-Hz displays via mpv `display-resample` and `interpolation` (without VapourSynth).

## Description
This feature was removed as part of an OSS UI simplification (2026). The app no longer exposes a general `video-sync=display-resample`, `interpolation`, or `tscale` preference. The legacy SQLite key `video_mpv_smooth` is not read or written. Smoother motion intent now lives entirely in [26-sixty-fps-motion](26-sixty-fps-motion.md), which interpolates content frames via VapourSynth at ~1.0× playback.

## Behavior

```gherkin
@status:removed @priority:p2 @layer:mpv @area:smooth-display
Feature: Smooth playback via mpv display-resample (removed)

  Scenario: Legacy SQLite key is ignored
    Given an old database contains video_mpv_smooth from earlier releases
    When Rhino starts or saves preferences today
    Then video_mpv_smooth is not read or written
    And mpv display-resample / interpolation defaults are unchanged

  Scenario: Behavioural replacement points to feature 26
    Given the user wants smoother motion on a fixed-Hz display
    When they enable Smooth Video (~60 FPS at 1.0×)
    Then expectations follow the VapourSynth FlowFPS rules in 26-sixty-fps-motion
    And no removed mpv interpolation toggle is exposed
```

## Notes
- Archived research ideas (tscale variants, VRR, battery saver profile) are intentionally not on the roadmap.
- See [VapourSynth](https://www.vapoursynth.com/), [mpv interpolation](https://mpv.io/manual/master/#options-interpolation), and [mpv video-sync](https://mpv.io/manual/master/#options-video-sync) for context.
