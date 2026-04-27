# ~60 fps motion (optional, VapourSynth)

---
status: done
priority: p1
layers: [mpv, ui, db, fs]
related: [10, 14, 25, 28]
actions: [smooth-60, vs-custom]
settings: [video_smooth_60, video_vs_path, video_mvtools_lib]
mpv_props: [vf, speed, hwdec, vd-lavc-dr, time-pos]
---

## Use cases
- Viewers who want more temporal smoothness ("soap opera" / HFR look) on a ~60 Hz display.
- Reuse the bundled MVTools FlowFPS path without learning VapourSynth.
- Plug in a custom `.vpy` script (e.g. RIFE) when desired.

## Description
A `vapoursynth` mpv filter runs a `.vpy` script that synthesizes ~60 fps frames from the source. The bundled `data/vs/rhino_60_mvtools.vpy` uses MVTools FlowFPS at full CPU affinity with a deeper VapourSynth queue and `LoadPlugin` resolved from `RHINO_MVTOOLS_LIB`. The user toggles intent via main menu **Preferences → Smooth Video (~60 FPS at 1.0×)**; the filter runs only at mpv `speed` ~1.0×. A custom `.vpy` path may replace the bundled script.

When the toggle is on but prerequisites (MVTools, vapoursynth-capable mpv build) are missing, the app keeps Smooth 60 off and opens a setup dialog with copy-paste install instructions. Once resolved, the absolute `libmvtools.so` path is cached in `video_mvtools_lib` to avoid re-scanning.

## Behavior

```gherkin
@status:done @priority:p1 @layer:mpv @area:smooth-60
Feature: Optional ~60 fps motion via VapourSynth

  Scenario: Toggle applies vf only at ~1.0× with MVTools resolved
    Given video_smooth_60 is true and libmvtools.so resolves successfully
    When media is loaded and mpv speed is approximately 1.0×
    Then vf contains the vapoursynth filter line with buffered-frames=24 and concurrent-frames=auto

  Scenario: Faster speeds skip vf without clearing user intent
    Given video_smooth_60 remains true in preferences
    When mpv speed is 1.5× or 2.0×
    Then the vapoursynth vf is omitted
    And video_smooth_60 stays true so 1.0× re-applies the filter

  Scenario: Incomplete setup opens helper dialog
    Given the user enables Smooth Video or selects a custom script while MVTools or vapoursynth-capable mpv is missing
    When vf application fails or prerequisites are absent
    Then video_smooth_60 is set to false
    And the setup dialog opens with install commands for VapourSynth, vsrepo, MVTools, and a vapoursynth-enabled mpv

  Scenario: Custom script row appears when video_vs_path is set
    Given video_vs_path is non-empty
    When the user opens the Preferences submenu
    Then a row shows the script basename next to a checkbox vs-custom
    And unchecking that row clears video_vs_path and reverts to the bundled .vpy

  Scenario: Paused seek temporarily clears vf for still frame
    Given a vapoursynth filter graph is active and the player is paused
    When the user seeks
    Then the app temporarily clears vf so mpv renders a normal still frame
    And Smooth 60 is reapplied on the next unpause if the preference remains enabled

  Scenario: Speed change resyncs RHINO_PLAYBACK_SPEED before vf rebuild
    Given the user selects a speed row from the header list
    When the idle chain reapplies vf
    Then RHINO_PLAYBACK_SPEED equals the row value
    And FlowFPS retiming uses that exact value before vf is rebuilt

  Scenario: Source rate ≥ 60 skips MVTools
    Given the source frame rate × speed in rational form is at least 60/1
    When the bundled script runs
    Then FlowFPS is skipped for that combination

  Scenario: 60000/1001 source still gets FlowFPS
    Given a source tagged at 60000/1001 (~59.94 fps) with speed 1.0
    When the bundled script runs
    Then FlowFPS still runs for that source
    And the rate is not collapsed to 60 by a float-with-epsilon shortcut

  Scenario: vf cache resolves libmvtools.so without rescanning
    Given video_mvtools_lib points at an existing libmvtools.so
    When the app installs the vapoursynth filter
    Then it uses the cached path without searching disk
    And RHINO_MVTOOLS_LIB takes precedence when set in the environment

  Scenario: Disable clears vf and restores hwdec / vd-lavc-dr
    Given a vapoursynth graph is active
    When the user disables Smooth Video or speed leaves ~1.0×
    Then vf is cleared
    And hwdec and vd-lavc-dr are restored to auto
```

## Notes
- Settings: `video_smooth_60` 0/1, `video_vs_path` UTF-8 path (empty for bundled), `video_mvtools_lib` cached absolute path. Persisted with other video prefs (see [14-preferences](14-preferences.md)).
- Menu wiring: stateful `smooth-60` action (Preferences → Smooth Video (~60 FPS at 1.0×)); `vs-custom` row appears when `video_vs_path` is non-empty; **Choose VapourSynth Script…** sets the path and saves only after MVTools is resolved.
- mpv defaults are otherwise untouched (no forced `video-sync`, `interpolation`, or `hwdec`); GTK frame clock and libmpv presentation paths still drive vsync.
- The vf line is `vapoursynth:file=…:buffered-frames=24:concurrent-frames=auto`. Deeper queues give MVTools more headroom at the cost of memory and seek latency.
- Before adding the vf, force `hwdec=no` and `vd-lavc-dr=no` (direct rendering can bypass the CPU filter path).
- After loadfile, run `apply_mpv_video` on the first GLib idle, then chain a second idle that re-applies if the vf line is still missing while Smooth 60 is on and speed ~1.0×. After an actual vf clear/replace, `seek <time-pos> absolute+keyframes` re-aligns A/V (skipped when `time-pos` < ~0.12 s); on seek failure, set `time-pos`.
- The bundled script tags source with `AssumeFPS` once at 1.0×, then `FlowFPS(60/1)` (no second `AssumeFPS(×1)`). At 1.5× / 2.0× it retimes with `AssumeFPS(× speed)` first, then `FlowFPS`.
- `libmvtools.so` resolution order: `RHINO_MVTOOLS_LIB` env, then cached `video_mvtools_lib` if still a file, then a bounded search (common distro paths, pipx / vsrepo under `~/.local`). On success the absolute path is saved to `video_mvtools_lib` and printed to stderr (e.g. `libmvtools -> …`).
- Subtitles (libass) are rendered outside the VapourSynth graph; A/B test by watching motion (pans), or briefly turn subs off.
- Default when DB has no relevant keys: Smooth 60 on with the bundled FlowFPS script. 1080p remains CPU-bound.
- See [25-smooth-playback](25-smooth-playback.md) (removed mpv display-resample path), [VapourSynth](https://www.vapoursynth.com/), [MVTools](https://github.com/dubhater/vapoursynth-mvtools), [RIFE](https://github.com/HolyWu/vs-rife).
