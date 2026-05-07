# ~60 fps motion (optional, VapourSynth)

---
status: done
priority: p1
layers: [mpv, ui, db, fs]
related: [10, 14, 25, 28]
actions: [smooth-60, vs-custom]
settings: [video_smooth_60, video_vs_path, video_mvtools_lib, video_smooth_max_area]
mpv_props: [vf, speed, hwdec, vd-lavc-dr, time-pos]
---

## Use cases
- Viewers who want more temporal smoothness ("soap opera" / HFR look) on a ~60 Hz display.
- Reuse the bundled MVTools FlowFPS path without learning VapourSynth.
- Plug in a custom `.vpy` script (e.g. RIFE) when desired.

## Description
A `vapoursynth` mpv filter runs a `.vpy` script that synthesizes ~60 fps frames from the source. The bundled `data/vs/rhino_60_mvtools.vpy` uses MVTools FlowFPS at full CPU affinity with a deeper VapourSynth queue and `LoadPlugin` resolved from `RHINO_MVTOOLS_LIB`. Rhino passes a **persisted pixel-area budget** (`video_smooth_max_area`, default ~1920×1080): when decode width×height exceeds that budget, the script scales **both** dimensions by √(budget ÷ decode area) so motion estimation stays within budget (**no** upscale to full decode). While Smooth is on with the bundled script, the app samples **this process’s** CPU on the existing 1 Hz transport tick; if utilization stays above **75%** of logical cores for two consecutive ticks, it recomputes the budget as **baseline × cores × 0.75 ÷ busy-core-equivalent**, persists it, and rebuilds `vf`. The user toggles intent via main menu **Preferences → Smooth Video (60 FPS)**; the filter runs only at mpv `speed` ~1.0×. A custom `.vpy` path may replace the bundled script (CPU budget adaptation applies only to the bundled path).

When the toggle is on but prerequisites (MVTools, vapoursynth-capable mpv build) are missing, the app keeps Smooth 60 off and opens a setup dialog with copy-paste install instructions. Once resolved, the absolute `libmvtools.so` path is cached in `video_mvtools_lib` to avoid re-scanning.

## Behavior

```gherkin
@status:done @priority:p1 @layer:mpv @area:smooth-60
Feature: Optional ~60 fps motion via VapourSynth

  Scenario: No stored preference keeps Smooth Video off
    Given the persistent store has no entry for the smooth-motion preference
    When the application starts
    Then the smooth-motion preference is off

  Scenario: Toggle applies vf only at ~1.0× with MVTools resolved
    Given the smooth-motion preference is on and the MVTools plugin resolves successfully
    When media is loaded and playback speed is approximately 1.0×
    Then the temporal-smoothing filter graph runs against the resolved MVTools plugin

  Scenario: Faster speeds skip vf without clearing user intent
    Given video_smooth_60 remains true in preferences
    When mpv speed is a fixed step other than approximately 1.0×
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

  Scenario: Pause without seek keeps temporal smoothing loaded
    Given Smooth Video is enabled for approximately 1.0× playback
    When playback is paused and playback position is not changed before resuming
    Then the temporal-smoothing filter graph stays loaded through pause and resume

  Scenario: Seek while paused unloads smoothing until playback resumes
    Given Smooth Video is enabled for approximately 1.0× playback
    When playback is paused and the viewer changes playback position before resuming
    Then the temporal-smoothing filter graph is unloaded before the position change
    And when playback resumes while still enabled the temporal-smoothing graph becomes active again

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

  Scenario: Source rate reported by container is recovered when not forwarded to the smoothing graph
    Given the source frame rate is constant
    And the smoothing graph receives an empty cadence from its input
    When the bundled script runs
    Then it recovers the source frame rate from the playback engine's container metadata
    And smoothing produces output at approximately 60 frames per second in sync with audio

  Scenario: Truly variable-frame-rate source falls back to passthrough
    Given the source frame rate is genuinely variable
    And the playback engine reports no container frame rate
    When the bundled script runs
    Then smoothing is skipped for that file
    And audio and video stay in sync

  Scenario: Cached MVTools plugin path skips a fresh search
    Given the persistent store has a cached path to the MVTools plugin and the file still exists
    When the app installs the temporal-smoothing filter graph
    Then it uses the cached path without searching disk
    And an explicit override in the environment takes precedence when set

  Scenario: Decode area above the saved budget scales motion estimation proportionally
    Given the smooth-motion preference is on with the bundled script
    When the decode width times height exceeds the saved pixel-area budget
    Then the bundled script scales both dimensions so motion estimation stays within that budget
    And the viewer sees the scaled raster without enlargement to full decode dimensions

  Scenario: Sustained high player CPU load lowers the saved pixel budget
    Given the smooth-motion preference is on with the bundled script
    And playback is progressing without the decoder core stalled idle
    When this player process repeatedly exceeds about three quarters of the machine's logical-core capacity
    Then the persistent store records a smaller pixel-area budget derived from that overload
    And the temporal-smoothing filter graph rebuilds with the new budget

  Scenario: Disable clears vf and restores hwdec / vd-lavc-dr
    Given a vapoursynth graph is active
    When the user disables Smooth Video or speed leaves ~1.0×
    Then vf is cleared
    And hwdec and vd-lavc-dr are restored to auto
```

## Notes
- Settings: `video_smooth_60` 0/1, `video_vs_path` UTF-8 path (empty for bundled), `video_mvtools_lib` cached absolute path, **`video_smooth_max_area`** (integer width×height budget for bundled MVTools ME/output raster before proportional downscale; default **2073600** = **1920×1080**). Legacy `video_manipmv_lib` may remain in the DB but is unused by the bundled script. Persisted with other video prefs (see [14-preferences](14-preferences.md)).
- Menu wiring: stateful `smooth-60` action (Preferences → Smooth Video (60 FPS)); `vs-custom` row appears when `video_vs_path` is non-empty; **Choose VapourSynth Script…** sets the path and saves only after MVTools is resolved.
- mpv defaults are otherwise untouched when Smooth is off (no forced `video-sync`, `interpolation`, or `hwdec`); GTK frame clock and libmpv presentation paths still drive vsync.
- The vf line is `vapoursynth:file=…:buffered-frames=16:concurrent-frames=auto` (fixed queue depth). Before **`vf add`**, Rhino sets **`RHINO_SMOOTH_MAX_AREA`** from **`video_smooth_max_area`** (clamped ≥ **320×180**). The bundled script **stderr** logs **`smooth_cap=`** and **`path=full`** (decode ≤ budget) or **`path=scaled`** (√ scale to fit budget). **`tier=uhd`** / **`tier=hd`** still reflect decode **≥2560×1440** vs smaller (**logging only**). **Bundled-only**: the **1 Hz** transport tick calls **`getrusage(RUSAGE_SELF)`** deltas → core-equivalent load; if **>** **0.75** × logical cores for **2** consecutive ticks, **`smooth_max_area` ← round(`2073600 × cores × 0.75 / busy_core_equiv`)** (clamped), **`save_video`**, **`apply_mpv_video`** (rebuild **`vf`**). Custom **`.vpy`** skips adaptation. Shared MVTools preset: **`blksize=128`**, **`overlap=32`**, **`Super.levels`** automatic (**`0`**). **`chroma=True`** on **Super** and **Analyse**. **`pel=1`**, **`sharp=1`**, **`search=4`** (hex), **`truemotion=True`**, **`global=True`** on **`Analyse`**, **`FlowFPS`** **`mask=2`**, **`blend=True`**, toward **60/1**. Stderr logs **`tier=`**, **`path=`**, **`decode=`**, **`me=`**. Details: [references-mvtools-super-levels](../references-mvtools-super-levels.md).
- Pause (`pause=yes`) alone keeps the `vf` graph loaded so pause/unpause does not rebuild MVTools. Each **main-player** seek (bottom bar, arrow keys, preview commit) runs **`vf clr`** immediately before **`seek`** whenever vapoursynth was still present; **`smooth_vf_attach_if_playing`** after the seek **only** when **not** paused (during playback scrubbing FlowFPS comes back at once). While **paused**, the graph stays cleared after that seek until **`Pause(false)`** reapplies it. **`apply_pending_resume`** also clears vapoursynth before its **`seek`** so resume positioning does not run through a stale graph. Playback (`pause=no`): **`smooth_vf_attach_if_playing`** re-runs filter application when Smooth applies and **`vapoursynth` is missing**. On **`Pause(false)`** it uses **`apply_mpv_video_after_transport_unpause`** / **`reapply_60_after_transport_unpause`**, which assume **`pause=no`** inside **`apply_mpv_video_impl`** — **`get_property("pause")`** can still lag **`observe_property`** after unpause, leaving **`use_mvtools`** false so Smooth never reattached after pause/seek. **`trust_not_paused`** only skips pause reads in the outer attach gates; seek-driven reattach uses live **`pause`**. Implemented from libmpv `pause` observation plus seek hooks.
- After loadfile, one GLib **idle** runs [apply_mpv_video] so **`vf`** / **`RHINO_PLAYBACK_SPEED`** / **`RHINO_SOURCE_FPS`** align once `path` and playback state are ready; no deliberate playback deferral before attaching Smooth **`vf`**. The separate ~**320 ms** file-loaded hook only aligns UI speed / subtitles — not Smooth timing.
- The bundled script tags source with `AssumeFPS` once at 1.0×, then `FlowFPS(60/1)` (no second `AssumeFPS(×1)`). At 1.5× / 2.0× it retimes with `AssumeFPS(× speed)` first, then `FlowFPS`.
- mpv's `vapoursynth` vf often forwards `video_in.fps_num=0 / fps_den=0` even for plain CFR mp4s (29.97 / 23.976 / 30). When **Rhino** runs `vf add`, **`RHINO_SOURCE_FPS`** is set first from mpv's **`container-fps`** (else **`estimated-vf-fps`**); the script prefers that **before** inferring from frame props. Props inference (`_Duration` / `_AbsoluteTime` deltas → `fps = 1e9/delta`, CFR-stable on frames 0–1, sanity 0.5–120 Hz) runs **only if env is unset** — needed for shell mpv, but **must not precede env**: decoder timestamps often track **display** cadence (~59.94 Hz) while film is 23.976; props-first wrongly made `fps×speed ≥ 60` and **skipped FlowFPS** entirely. Env fallback still uses `Fraction(...).limit_denominator(1001)`. When mpv has no usable fps and no env (true VFR), passthrough avoids A/V drift.
- MVTools plugin resolution order: `RHINO_MVTOOLS_LIB` env, then cached `video_mvtools_lib` if still a file, then a bounded search. On Linux that search covers common distro paths and pipx / vsrepo under `~/.local`; on macOS it covers the Homebrew prefixes (`/opt/homebrew/lib/libmvtools.dylib` on Apple Silicon, `/usr/local/lib/libmvtools.dylib` on Intel — `brew install mvtools`) and skips the Linux-only `~/.local` walk. On success the absolute path is saved to `video_mvtools_lib` and printed to stderr (e.g. `libmvtools -> …`).
- macOS binding: the plugin file is `libmvtools.dylib` (Linux: `libmvtools.so`); `paths::MVTOOLS_FILE` and `DISTRO_MVTOOLS_PATHS` are gated with `cfg(target_os = "macos")`. The bundled `rhino_60_mvtools.vpy` mirrors the same Homebrew lookup at `LoadPlugin` time. Homebrew’s `mpv` formula (0.41+) lists VapourSynth as a build dependency, so the same `libmpv` Rhino links against can run the bundled script. `app/vs_setup_dialog` shows a macOS-specific instruction text (`brew install mpv mvtools`) at runtime via `cfg(target_os = "macos")`.
- Subtitles (libass) are rendered outside the VapourSynth graph; A/B test by watching motion (pans), or briefly turn subs off.
- Default when the persistent store has no `video_smooth_60` row: **off** (bundled FlowFPS script is used only after the user turns the option on). 1080p remains CPU-bound when enabled.
- See [25-smooth-playback](25-smooth-playback.md) (removed mpv display-resample path), [VapourSynth](https://www.vapoursynth.com/), [MVTools](https://github.com/dubhater/vapoursynth-mvtools), [RIFE](https://github.com/HolyWu/vs-rife).
