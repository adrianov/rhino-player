# ~60 fps motion (optional, VapourSynth)

---
status: done
priority: p1
layers: [mpv, ui, db, fs]
related: [10, 14, 25, 28]
actions: [smooth-60, vs-custom]
settings: [video_smooth_60, video_vs_path, video_mvtools_lib, video_smooth_max_area]
mpv_props: [vf, speed, hwdec, vd-lavc-dr, time-pos, video-sync, interpolation, mistimed-frame-count, frame-drop-count, decoder-frame-drop-count, vo-delayed-frame-count, container-fps, estimated-vf-fps]
---

## Use cases
- Viewers who want more temporal smoothness ("soap opera" / HFR look) on a ~60 Hz display.
- Reuse the bundled MVTools FlowFPS path without learning VapourSynth.
- Plug in a custom `.vpy` script (e.g. RIFE) when desired.

## Description
A `vapoursynth` mpv filter runs a `.vpy` script that synthesizes ~60 fps frames from the source. The bundled `data/vs/rhino_60_mvtools.vpy` uses MVTools FlowFPS at full CPU affinity with a deeper VapourSynth queue and `LoadPlugin` resolved from `RHINO_MVTOOLS_LIB`. Rhino passes a **persisted pixel-area budget** (`video_smooth_max_area`, default **exactly** **1920×1080** px² = **2073600**): when decode width×height exceeds that budget, the script proportionally **downscales for motion estimation** (√(budget ÷ decode area)), runs **FlowFPS** on that ME raster, and **outputs at that raster size** (~60 fps temporal smoothing; geometry may be smaller than the decoded picture, with the player scaling to the window). While Smooth is on with the bundled script, the app reads the playback engine’s **presentation strain** tallies once per **1 Hz** transport heartbeat (preferring **frames missing ideal display cadence**, else **output frames that were not shown**, else **decoder‑side skipped frames** when the former are unavailable). **Overload** fires after strict rolling strain **exceeds about twenty percent** of the pacing reference for **roughly five successive heartbeats** (trailing **≈5 s** window); **recovery** fires after relaxed rolling strain **stays below about ten percent** for **roughly three hundred successive heartbeats**. **Recovery does not widen** the persisted ME budget toward the nominal ceiling once **overload has already stepped it down** on the **same open media**, until playback moves to another clip (**new opening** clears that lock). Recovery is **also withheld** on a heartbeat when the **viewer process** averaged **greater than ~75%** of **nominal parallel processor capacity** over the preceding heartbeat interval (**process CPU seconds ÷ wall seconds ÷ logical processor count**) so raising motion estimation stays off while the viewer is saturated. **Heartbeats while the playback window is inactive, hidden, unmapped, minimized, or withdrawn do not advance overload or recovery**, so compositor pacing in the background does not skew the budget. On overload the saved budget steps **down** about **10%** and the filter rebuild persists that cap; on recovery the budget steps **up** about **10%** toward the **recovery ceiling**—decoded width×height when those dimensions are known, otherwise the same nominal reference area as a fresh install (**1920×1080**)—only when **still below** that ceiling **and decode width×height** still exceeds the persisted **cap** (motion estimation stays downscaled), with the graph reapplied—same pacing as overload (no raise on the tick that shrunk); **when the decoded raster already fits the persisted cap**, recovery does **not** increase the motion-estimation budget. This adaptation applies **only** to the bundled `.vpy` when no custom script path is set. The user toggles intent via main menu **Preferences → Smooth Video (60 FPS)**; the filter runs only at mpv `speed` ~1.0×. A custom `.vpy` path may replace the bundled script (ME budget adaptation skipped for custom paths). For the bundled path, the app also keeps a **global** default motion-estimation pixel budget in the persistent store; **per opened file** it can store that file’s decode width and height and an optional **file-specific** motion-estimation budget. When a file has no saved file-specific budget, the effective starting budget is the **closest prior opening** (by decode width and height among stored rows that already have a saved budget), and when no such row exists it uses the **global** default.

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

  Scenario: Seek keeps temporal smoothing loaded
    Given Smooth Video is enabled for approximately 1.0× playback
    When the viewer changes playback position
    Then the temporal-smoothing filter graph stays loaded through the position change
    And playback after the position change is still smoothed

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

  Scenario: Unstable frame cadence uses display resampling instead of VapourSynth
    Given the smooth-motion preference is on and playback speed is approximately 1.0×
    And the open item has unstable frame cadence (e.g. mixed broadcast rates on a disc title, or cadence jumps after a seek)
    When the playback engine reports cadence that is missing or not yet stable
    Then the temporal-smoothing filter graph is not loaded
    And presentation uses display-aligned resampling for smoother motion on a fixed-Hz display
    And the filter graph is not rebuilt on every cadence fluctuation

  Scenario: Stable disc cadence may use VapourSynth after settle
    Given the smooth-motion preference is on and a Blu-ray title reports a stable container frame rate for several consecutive reads
    When cadence no longer jumps between broadcast rates
    Then the bundled temporal-smoothing filter may run at approximately 1.0× like a local file

  Scenario: DVD chapter file without container frame rate uses broadcast cadence for Smooth
    Given the smooth-motion preference is on and playback speed is approximately 1.0×
    And the open item is a DVD chapter file under a video transport folder
    And the playback engine reports no container frame rate for that file
    When decode height indicates a PAL DVD raster
    Then the temporal-smoothing filter graph may run at approximately 1.0× using the PAL broadcast rate
    And presentation does not stay on display-aligned resampling only because cadence was unknown

  Scenario: Cached MVTools plugin path skips a fresh search
    Given the persistent store has a cached path to the MVTools plugin and the file still exists
    When the app installs the temporal-smoothing filter graph
    Then it uses the cached path without searching disk
    And an explicit override in the environment takes precedence when set

  Scenario: Decode area above the saved budget scales motion estimation proportionally
    Given the smooth-motion preference is on with the bundled script
    When the decode width times height exceeds the saved pixel-area budget
    Then the bundled script scales both dimensions for motion estimation so that raster stays within that budget
    And interpolated frames leave the filter graph at the motion-estimation raster size when that raster is smaller than the decoded picture

  Scenario: File-specific motion-estimation budget overrides the global default when reopening the same file
    Given the smooth-motion preference is on with the bundled script
    And the persistent store holds a motion-estimation pixel budget recorded for this opened file
    When that file is opened again for playback
    Then the temporal-smoothing filter graph uses that file-specific budget instead of the global default

  Scenario: New file without a saved file budget picks the closest prior opening by decode dimensions
    Given the smooth-motion preference is on with the bundled script
    And the persistent store holds decode dimensions and a saved motion-estimation budget from another opened file
    And the current opened file has no saved file-specific motion-estimation budget
    When the bundled script is applied for the current decode width and height
    Then the starting motion-estimation pixel budget matches the saved budget from the prior opening whose stored decode dimensions are closest to the current decode dimensions

  Scenario: New file falls back to global default when no prior opening supplies a saved budget
    Given the smooth-motion preference is on with the bundled script
    And no row in the persistent store for other files supplies both stored decode dimensions and a saved motion-estimation budget usable for matching
    When the bundled script is applied
    Then the starting motion-estimation pixel budget is the global default from preferences

  Scenario: Sustained elevated smooth-playback strain rate lowers the saved pixel budget
    Given the smooth-motion preference is on with the bundled script
    And playback is progressing without the decoder core stalled idle
    When presentation strain at the output exceeds about twenty percent of the pacing reference over a trailing window of roughly five successive seconds throughout five successive transport-heartbeat evaluations
    Then the persistent store records a smaller pixel-area motion-estimation budget in steps about one tenth below each prior saved value floored at the persistence minimum
    And the temporal-smoothing filter graph is reapplied so the smoothing path observes that new pixel-area budget

  Scenario: Mild rolling presentation strain sustained for hundreds of heartbeats raises the saved pixel budget
    Given the smooth-motion preference is on with the bundled script
    And playback is progressing without the decoder core stalled idle
    And overload has not lowered the persisted motion-estimation pixel-area budget on this open media
    And averaged processor occupancy for the viewer process stays at or below about three quarters of nominal parallel capacity across recent transport heartbeats
    And the persisted pixel-area budget is below the recovery ceiling for the current decoded raster
    And decode width times height exceeds that persisted pixel-area budget
    When rolling presentation strain stays below about ten percent of the pacing reference for roughly three hundred successive transport-heartbeat evaluations
    Then the persistent store records a larger pixel-area budget in steps about one tenth above each prior saved value capped at that recovery ceiling
    And each successful raise reapplies the temporal-smoothing filter graph during the same viewing session so the smoothing path observes that new pixel-area budget

  Scenario: Recovery does not widen ME budget after overload stepped it down on the same open media
    Given the smooth-motion preference is on with the bundled script
    And playback is progressing without the decoder core stalled idle
    And the persisted pixel-area budget is below the recovery ceiling for the current decoded raster
    And decode width times height exceeds that persisted pixel-area budget
    And overload has already lowered the persisted motion-estimation pixel-area budget on this open media
    When rolling presentation strain stays below about ten percent of the pacing reference for roughly three hundred successive transport-heartbeat evaluations
    Then the persisted motion-estimation pixel-area budget is unchanged over that window from recovery widening

  Scenario: Recovery does not widen ME budget while the viewer process saturates most nominal parallel capacity
    Given the smooth-motion preference is on with the bundled script
    And playback is progressing without the decoder core stalled idle
    And overload has not lowered the persisted motion-estimation pixel-area budget on this open media
    And the persisted pixel-area budget is below the recovery ceiling for the current decoded raster
    And decode width times height exceeds that persisted pixel-area budget
    And averaged processor occupancy for the viewer process stays above about three quarters of nominal parallel capacity across recent transport heartbeats
    When rolling presentation strain stays below about ten percent of the pacing reference for roughly three hundred successive transport-heartbeat evaluations
    Then the persisted motion-estimation pixel-area budget is unchanged over that window from recovery widening

  Scenario: Quiet presentation strain does not raise budget when motion estimation matches native decoded size
    Given the smooth-motion preference is on with the bundled script
    And playback is progressing without the decoder core stalled idle
    And decode width multiplied by decode height fits within the persisted pixel-area motion-estimation budget
    When rolling presentation strain stays below about ten percent of the pacing reference for roughly three hundred successive transport-heartbeat evaluations
    Then the persisted pixel-area motion-estimation budget is unchanged by recovery

  Scenario: Strain ladders skip heartbeats while the playback window is unavailable for pacing
    Given the smooth-motion preference is on with the bundled script
    And playback is progressing without the decoder core stalled idle
    When the playback window is inactive, minimized, withdrawn, invisible, or not mapped for viewer interaction with the playback shell
    Then that transport-heartbeat evaluation does not advance overload or recovery toward changing the persisted motion-estimation pixel-area budget

  Scenario: Disable clears vf and restores hwdec / vd-lavc-dr
    Given a vapoursynth graph is active
    When the user disables Smooth Video or speed leaves ~1.0×
    Then vf is cleared
    And hwdec and vd-lavc-dr are restored to auto

  Scenario: Disabling Smooth keeps the same open media at about the same position
    Given a vapoursynth graph is active on opened media
    When the user disables Smooth Video
    Then the viewer stays on the same open media from approximately the same playback position
    And plain playback timing applies without the temporal-smoothing graph

  Scenario: Re-enabling Smooth at the current position keeps audio and video aligned
    Given smooth motion was active on opened media at approximately normal playback speed
    When the user disables smooth motion and enables it again without changing playback position
    Then audio and video remain aligned at about the same position

  Scenario: Enable smooth during playback keeps audio and video aligned
    Given the smooth-motion preference is off
    And opened media is playing at approximately normal playback speed
    When the viewer enables smooth motion without changing playback position
    Then audio and video remain aligned at about the same position

  Scenario: Opening media with smooth motion on stays aligned after resume
    Given the smooth-motion preference is on at approximately normal playback speed
    When the viewer opens media from a saved resume position
    Then temporal smoothing attaches only after the resume position is applied
    And audio and video remain aligned at that position
```

## Notes
- Settings: `video_smooth_60` 0/1, `video_vs_path` UTF-8 path (empty for bundled), `video_mvtools_lib` cached absolute path, **`video_smooth_max_area`** (integer width×height budget for bundled MVTools ME/output raster before proportional downscale; default **2073600** = **1920×1080**). Legacy `video_manipmv_lib` may remain in the DB but is unused by the bundled script. Persisted with other video prefs (see [14-preferences](14-preferences.md)). **Per-file** data lives on the same `media` rows as resume and duration: optional **`decode_w`**, **`decode_h`**, **`smooth_me_budget_px2`**, and **`smooth_me_budget_updated_at`** (Unix ms when the px² value was last written; tie-break among exact-dimension rows). Effective bundled ME px² is **this** file’s **`smooth_me_budget_px2`** when set (clamped to **`MIN_SMOOTH_MAX_AREA`**); otherwise **`smooth_me_budget_px2`** from **another** row whose **`decode_w`** and **`decode_h`** **equal** the current decode dimensions (tie: greater **`smooth_me_budget_updated_at`**, then **`rowid`**); **neighbor** values may be **below or above** **`video_smooth_max_area`**; **only** when decode size is unknown or **no** such row exists use **`video_smooth_max_area`**. Adaptive overload persists both the global setting and the current file’s **`smooth_me_budget_px2`**. Decode dimensions are refreshed on the transport heartbeat when known. Implementation: **`src/db/media_me_budget.rs`**, **`src/video_pref/smooth_me_budget_resolve.rs`**. Verbose ME budget tracing (**`RHINO_VIDEO_LOG=1`**): **`ME resolve effective_px²=`**, **`persist_budget media_save`**, **`RHINO_SMOOTH_MAX_AREA`** before **`vf add`**.
- **Debug:** **`RHINO_SMOOTH_DROP_STATS=1`** — stderr **≈every 5 s** while bundled Smooth **`vf`** is active (**`smooth_budget`**): **`[rhino] smooth: stats`** **Δ** and totals for **`mistimed-frame-count`**, **`frame-drop-count`**, **`decoder-frame-drop-count`**, which **signal** drives the budget (**mistimed** → **VO** → **decoder**), wall interval, and **~%** vs the same **denominator Hz** as overload (**mistimed** / **VO**: **≥ ~60 Hz**; **decoder**: **`container-fps`×`speed`** or **`estimated-vf-fps`**). **`stats`** **`eprintln`** is **skipped** once overload has shrunk ME on this **open media** **and** the strict-window strain rate is **below** the **overload** firing band (**~20%**); the **5 s** bookkeeping still advances so the next visible line is not a long catch-up. **`smooth_budget`** itself (decision logs + persistence) runs **≈once per transport tick** only while the shell window is **visible**, **mapped**, **active**, and not **`GdkToplevel::MINIMIZED`** (**`transport_events/deferred_resync.rs`** **`smooth_budget_transport_window_ticks_count`**). **Besides** that sampler, **`[rhino] smooth: decision …`** logs **`overload`**, **`raise`** only when ME widens, **`persist_skip`**, and anomalies—**`raise_skipped`** lines are **not** emitted; recovery arms silently when decode fits cap, overload lock, CPU gate, or no step.
- **mpv log surfacing** (always-on, **`transport_observe_install.rs`**): the main mpv handle requests **warn**-level log messages (**`mpv_request_log_messages`**) and prints them as **`[rhino] mpv: [prefix] level: text`** — this is what exposes the real reason behind opaque command errors (e.g. the VSScript Python-finalize fatal behind **`MPV_ERROR_COMMAND`**), decoder hiccups, and mpv's own A/V-desync warnings. The vapoursynth re-init pair on every seek (`Frame requested during init` / `black dummy frame`) is expected and printed once per process, then muted.
- Menu wiring: stateful `smooth-60` action (Preferences → Smooth Video (60 FPS)); `vs-custom` row appears when `video_vs_path` is non-empty; **Choose VapourSynth Script…** sets the path and saves only after MVTools is resolved.
- **Linux** (`vo=libmpv` + GTK **`GLArea`**): Plain and Smooth-on both prefer **`video-sync=display-resample`**, **`interpolation=no`**, and **`report_swap`** when the atomic gate is on. **User toggles Smooth on while playing** (menu / toolbar, empty **`vf`**): **`smooth_user_enable_playing_reset`** — **`stop`+`loadfile replace`** at the current playhead (**`smooth-on loadfile reset (user toggle while playing)`** log), then debounced **`apply_mpv_video`** after **`FileLoaded`** / resume seek. **Reattach after a strip** (**`smooth_vf_stripped_this_open`** set — Smooth off→on, speed change; plain seeks no longer strip): **`smooth_reattach_after_vf_strip`** — **`defer_smooth_vf_swap`** only (keyframe seek + tail **`vf add`**, never **`loadfile`**); the one-shot reload fallback lives in **`add_smooth_60`**'s **`vf add`**-failure branch. **`add_smooth_60`** defers (no attach attempt) while a resume seek is pending — **`vf add`** during a fresh load fails with **`MPV_ERROR_COMMAND`**; the debounced transport resync self-retries after resume settles. **`vf add` runs exactly once per attach** (no retry loop, no `vf set` fallback — a rejected add is never transient; see the VSScript pin bullet for the historical root cause). **First smooth-on while paused**: live **`add_smooth_60`**. **Smooth-on after off** (stripped flag set by **`clear_vf`**): **`loadfile replace`** at the current playhead once (**`reload_open_media_for_vf_reset`**, **`smooth_vf_reload_attempted`**); else **`defer_smooth_vf_swap`** (**`smooth-reattach`**) — keyframe seek + **~1 s** tail, then **`VF_SWAP_POST_SEEK_ATTACH`** + **`vf add`**. If **`vf add`** still fails, reload is tried once before clearing the pref. **Graph replace** (cadence rebuild while **`vapoursynth`** still active): same defer path (**`smooth-swap`** tag) after **`vf remove`**. **Smooth off**: **`vf remove vapoursynth`** via **`clear_vf`**, **`absolute+keyframes`** via **`smooth_off_refresh_playhead`** (**`smooth-off`** log tag). Do **not** use **`vf clr`** / empty **`vf`** for vapoursynth teardown — mpv cannot **`vf add vapoursynth`** again on the same file after **`vf clr`**. **`VF_SWAP_DEFER_IN_FLIGHT`** blocks stacked defers. **`schedule_smooth_60_resync_idle`** waits for **`resume_seek_pending`**. **320 ms** file-loaded hook updates speed env only when the graph already matches prefs. (**no** Smooth-off **`loadfile`** — sibling-folder EOF false positives).
- **macOS** (`vo=libmpv` + **`CVDisplayLink`**): Same paths; vapoursynth **`vf remove`** and **`vf add`** use **`with_macos_vf_teardown`** when bundled.
- **VSScript runtime pin** (**`src/video_pref/vsscript_pin.rs`**): mpv's `vapoursynth` filter pairs VSScript API4 **`createScript()`** / **`freeScript()`** per instance; freeing the last script environment (Smooth off) finalizes VapourSynth's embedded **Python**, which cannot be re-initialized in-process — every later **`vf add vapoursynth`** then fails (`Failed to initialize the VapourSynth Python module for VSScript use`, surfaced as **`MPV_ERROR_COMMAND`**). **`pin_vsscript_python`** dlopens the VSScript library (**`getVSScriptAPI(4.1)`**) and holds one never-freed script environment before the first attach, so Python stays initialized and Smooth can toggle off→on freely. R76+ exports only API4 — there is no `vsscript_init` symbol anymore.
- **Bundled vapoursynth vf:** **Bundled `.vpy` ME cap:** **Bundled `.vpy`** reads **`RHINO_*` via robust host env** after Rust `set_var` — **Linux** and **macOS:** libc `getenv` (pointer-safe copy via **ctypes** — never `restype=c_char_p`). **`RHINO_SMOOTH_MAX_AREA`** is evaluated before mpv's optional **`user_data`** global so SQLite-driven overload steps cannot be masked by stale script state. Rhino attaches `vapoursynth:file=…:buffered-frames=N:concurrent-frames=auto:user-data=<px²>` when libmpv accepts **`user-data=`** (bundled script only); older mpv omits it — VS worker may then rely on **`RHINO_SMOOTH_MAX_AREA`** only. **N** is **`SMOOTH_VF_BUFFERED_FRAMES`** in **`src/video_pref/smooth_motion_tier.rs`**. Rhino skips **`vf clr`/`vf add`** only when **`vf_smooth_matches_prefs`** succeeds (resolved script, **`smooth_max_area_env_matches`**, **`user-data=`** matches persisted px² where bundled, **`smooth_vf_me_budget_applied.rs`** last-rebuild tracking). Bundled **`.vpy`** prefers mpv **`user_data`** over env when digits (**forked worker environ** can lag **`set_var`**). **stderr** includes **`smooth_cap=`**, **`path=`**, **`decode=`**, **`vf_out=`** — **`vf_out`** is the ME / **FlowFPS** raster without upscaling back to decode dimensions; geometry is aligned/cropped per **`blksize`** helpers in **`data/vs/rhino_60_mvtools.vpy`** (**`overlap` &lt; `blksize`** stays an MVTools rule). Decode-area **tier** tags for logging use **`_MV_DECODE_TAG_AREA`** beside the same script. **Bundled-only** **`video_smooth_max_area`** adjustments (**`smooth_budget.rs`**, transport tick ≈ **1 Hz**) use a **strain signal** from **mpv**: **`mistimed-frame-count`** when readable (display / resample cadence mismatch — primary for **`video-sync=display-resample`**), else **`frame-drop-count`** (VO output drops), else **`decoder-frame-drop-count`**, else budgeting is skipped entirely. Rolling **≈5 s**: **Δsignal / (denominator Hz × Δwall)** — **decoder** path uses estimated decode fps **`container-fps` × `speed`**, else **`estimated-vf-fps`**; **mistimed** / **VO** use **max(~60 Hz, that decode fps)** so ~24 Hz film alone does not inflate **rate**. **Overload**: rolling strain **>** **20%** (**`OVERLOAD_STRAIN_GT_FRAC`**) using the **same minimum trailing wall span as recovery** (**≥ ~2 s**, **`RECOVERY_STRAIN_TAIL_MIN_ELAPSED_SECS`**) before a rate exists (samples still trimmed to **≈5 s**, **`DROP_WINDOW_SECS`**); **five successive** **`1 Hz`** ticks ⇒ **`budget_after_decoder_overload`**: ~**10%** shrink (**`⌊saved×90+50⌋/100`**, at least **`saved−1`**, **`clamp_smooth_area`**, **`apply_mpv_video`**); logged strain rate is diagnostic only. **Cooldown**: deque trimmed after shrink. **Recovery**: relaxed rolling strain over **≥ ~2 s** wall (**`RECOVERY_STRAIN_TAIL_MIN_ELAPSED_SECS`**) stays **strictly \<** **10%** (**`RECOVERY_STRAIN_LT_FRAC`**) **300** successive ticks (**`RECOVERY_FIRE_STREAK_TICKS`**) — same ladder signal and denom Hz as overload’s rate math; omit **`recovery_candidate`** when decode **W×H** from **`video-params/w`**/**`h`** (fallback **`width`/`height`**) fits the clamped (**`MIN_SMOOTH_MAX_AREA`**) persisted cap — same **native vs scaled** split as **`smooth_me_geometry::bundled_me_vf_out_wh`**; **~10%**/step toward **`recovery_ceiling_px`** (**decoded W×H** when readable, else **`DEFAULT_SMOOTH_MAX_AREA`**) when still below that ceiling and a raise would widen ME; **unreadable** decode dimensions fall back to the default reference ceiling. Overload/recovery hysteresis resets when counter **drops** vs prior sample (reload/seek). **`vf clr`/`vf add`** runs when the persisted budget changes so **`smooth_cap`** stays aligned. **Recovery math:** while the saved budget is **strictly below** **`recovery_ceiling_px`**, each raise uses **`⌊saved×110+50⌋/100`**, at least **`saved+1`**, capped at that ceiling (~**10%**/step, integer half-up rounding). A tick that shrinks from overload does **not** also apply a raise. On **`FileLoaded`** / **`path`** (**`dispatch_sync_ui`**), Rhino calls **`forget_bundled_me_budget_vf_apply_on_new_media`** so **`vf_smooth_matches_prefs`** cannot skip reinstall after **`loadfile`**. **MVTools filter arguments** (**`Super`**, **`Analyse`**, **`FlowFPS`**, masking, cadence targets): **only** **`data/vs/rhino_60_mvtools.vpy`**; **`levels`** background: [references-mvtools-super-levels](../references-mvtools-super-levels.md).
- Pause (`pause=yes`) alone keeps the `vf` graph loaded so pause/unpause does not rebuild MVTools. **Main-player seeks keep the vapoursynth graph attached** (`seek_keyframes.rs` only marks the disc cadence gate): once mpv destroys a vapoursynth filter instance, every later **`vf add vapoursynth`** in the same process fails with **`MPV_ERROR_COMMAND`** (observed on macOS / Homebrew mpv + Python 3.14), so strip-before-seek poisoned reattach. The post-seek **debounced** tail (**`request_smooth_60_transport_resync`**, **~1 s** for arrow bursts) still runs **`apply_mpv_video`**, which no-ops via **`vf_smooth_matches_prefs`** when the surviving graph already matches. **`apply_pending_resume`** still clears vapoursynth before its **`seek`** (graph is normally absent there — attach is gated until resume completes). DVD global seeks (**`dvd_vob_timeline_transport`**) still strip before exact seeks across chapter files.
- After loadfile, one GLib **idle** runs [apply_mpv_video] so **`vf`** / **`RHINO_PLAYBACK_SPEED`** / **`RHINO_SOURCE_FPS`** align once `path` and playback state are ready; no deliberate playback deferral before attaching Smooth **`vf`**. The separate ~**320 ms** file-loaded hook only aligns UI speed / subtitles — not Smooth timing.
- The bundled script tags source with `AssumeFPS` once at 1.0×, then `FlowFPS(60/1)` (no second `AssumeFPS(×1)`). At 1.5× / 2.0× it retimes with `AssumeFPS(× speed)` first, then `FlowFPS`.
- mpv's `vapoursynth` vf often forwards `video_in.fps_num=0 / fps_den=0` even for plain CFR mp4s (29.97 / 23.976 / 30). When **Rhino** runs `vf add`, **`RHINO_SOURCE_FPS`** is set from mpv's **`container-fps`** (else **`estimated-vf-fps`** when trustworthy); the script prefers **`RHINO_SOURCE_FPS`** **before** inferring from frame props. **`estimated-vf-fps` is ignored** while a **`vapoursynth`** vf is active (it tracks output ~60 Hz), for **`bd://` / `bluray://`** paths, and for **`.vob`** chapter files under **`VIDEO_TS/`** (folder-open DVD). Those chapter files often omit **`container-fps`**; **`video_ext::dvd_vob_broadcast_fps`** supplies **25** Hz when decode height is **576** (PAL) or **30000/1001** when height is **464–486** (NTSC). Detection uses **`local_file_from_mpv`** (not raw `file://` strings). Decode size for inference follows **`video-params`**, then **`dwidth`/`dheight`** (same family as window aspect), then **`width`/`height`**. Disc cadence locks on the first plausible broadcast rate until **`path`** changes. **Unstable frame cadence** (code flag `interleaved_smooth` — **not** interlaced SD/DVD fields): **`smooth_prefers_display_resample`**, **`mark_smooth_cadence_unstable_after_seek`** on seek → **`video-sync=display-resample`**, no VapourSynth rebuild loop; **`bd://`** opens cautious until **`CADENCE_STABLE_READS`** plausible **`container-fps`**; a **>12%** cadence jump re-enters display-resample mode (mixed 23.976 / 29.97 on one title). Stable CFR segments may still attach MVTools. **Interlaced** PAL/NTSC DVD is unrelated — Smooth still targets **25** / **29.97** Hz CFR for chapter `.vob` files. Props inference (`_Duration` / `_AbsoluteTime` deltas → `fps = 1e9/delta`, CFR-stable on frames 0–1, sanity 0.5–120 Hz) runs **only if env is unset** — needed for shell mpv, but **must not precede env**: decoder timestamps often track **display** cadence (~59.94 Hz) while film is 23.976; props-first wrongly made `fps×speed ≥ 60` and **skipped FlowFPS** entirely. Env fallback still uses `Fraction(...).limit_denominator(1001)`. When mpv has no usable fps and no env (true VFR), passthrough avoids A/V drift.
- MVTools plugin resolution order: `RHINO_MVTOOLS_LIB` env, then cached `video_mvtools_lib` if still a file, then a bounded search. On Linux that search covers common distro paths and pipx / vsrepo under `~/.local`; on macOS it covers Homebrew **`vapoursynth-mvtools`** (`mvtools.dylib` under `…/vapoursynth/plugins/`) and legacy **`libmvtools.dylib`** under **`$(brew --prefix)/lib`** (`brew install vapoursynth-mvtools`). On success the absolute path is saved to `video_mvtools_lib` and printed to stderr (e.g. `libmvtools -> …`).
- macOS binding: the plugin file is **`mvtools.dylib`** (Homebrew **`vapoursynth-mvtools`**) or legacy **`libmvtools.dylib`** (Linux: `libmvtools.so`); [macos_mvtools_lib_search] in **`paths_mvtools_macos.rs`**. mpv loads **`libvapoursynth-script.dylib`**; Homebrew R76 ships **`libvsscript.dylib`** — Rhino **re-execs** once at startup with **`DYLD_LIBRARY_PATH`** (**`paths_vapoursynth_macos.rs`**) plus a legacy-name symlink under **`~/.config/rhino/dylib`**. The bundled `rhino_60_mvtools.vpy` mirrors the same Homebrew lookup at `LoadPlugin` time. Homebrew’s `mpv` formula (0.41+) lists VapourSynth as a build dependency, so the same `libmpv` Rhino links against can run the bundled script. `app/vs_setup_dialog` shows a macOS-specific instruction text (`brew install mpv vapoursynth vapoursynth-mvtools`) at runtime via `cfg(target_os = "macos")`.
- Subtitles (libass) are rendered outside the VapourSynth graph; A/B test by watching motion (pans), or briefly turn subs off.
- Default when the persistent store has no `video_smooth_60` row: **off** (bundled FlowFPS script is used only after the user turns the option on). 1080p remains CPU-bound when enabled.
- See [25-smooth-playback](25-smooth-playback.md) (removed mpv display-resample path), [VapourSynth](https://www.vapoursynth.com/), [MVTools](https://github.com/dubhater/vapoursynth-mvtools), [RIFE](https://github.com/HolyWu/vs-rife).
