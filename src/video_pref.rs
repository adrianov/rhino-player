//! Optional mpv VapourSynth `vf` from [crate::db::VideoPrefs].
//! See `docs/features/26-sixty-fps-motion.md`. Sets [crate::paths::RHINO_PLAYBACK_SPEED_VAR] from mpv
//! `speed` before the VapourSynth filter is built. The graph is **rebuilt on events**: after mpv
//! reports new media (**`FileLoaded`** and **`path`** change — coalesced to one idle — covering Open,
//! drag-drop, sibling EOF advance, **Previous** / **Next**, and `loadfile`), when the user picks
//! **playback speed** in the header (deferred idle), and after unpause when **`vapoursynth`** was
//! stripped for a **seek while paused** or similar. There is **no** periodic "watch" on `vf` for runtime plugin failures — `vf` add failure still
//! clears the pref at apply time; a script that dies *after* add is a rare install issue (toggle off in
//! **Preferences** or fix mvtools).
//! Set `RHINO_VIDEO_LOG=1` for per-step mpv result lines on stderr.
//!
//! If the VapourSynth `vf` cannot be added (no script, or mpv reports error — missing filter, plugin,
//! Python), [apply_mpv_video] sets `smooth_60` to `false`, saves settings, and returns `true` so the UI
//! can sync the **Smooth Video (60 FPS)** menu.
//!
//! When attaching Smooth `vf` with media open, Rhino leaves **`hwdec`** / **`vd-lavc-dr`** unchanged
//! (usually **`hwdec=auto`**).
//! **`buffered-frames=`** comes from **`SMOOTH_VF_BUFFERED_FRAMES`** (**`smooth_motion_tier.rs`**); **`mv.Super` /
//! `mv.Analyse` / `mv.FlowFPS`** tunables live in the bundled `.vpy`. Rhino passes **`RHINO_SMOOTH_MAX_AREA`**
//! from **`video_smooth_max_area`** (persisted; proportional ME downscale when decode exceeds it; **FlowFPS**
//! output stays at the ME raster — may diverge from decode dimensions — see `data/vs/rhino_60_mvtools.vpy`).
//! With the bundled script, **`smooth_budget_on_transport_tick`** may lower **`video_smooth_max_area`** when **this process**
//! sustains **>** ~**75%** logical-core CPU utilization (two consecutive **1 Hz** transport ticks), scaling from the
//! **current saved** budget (not the factory default), then rebuilds **`vf`**.
//! When **`FileLoaded`** or **`path`** fires (transport coalesced idle), **if** **`vf_smooth_matches_prefs`**
//! is true (resolved script · mpv **`buffered-frames=`** depth · **`RHINO_SMOOTH_MAX_AREA`**, SQLite **`video_smooth_max_area`**,
//! and last **successful** bundled ME rebuild tracked in **`smooth_vf_me_budget_applied.rs`**), Rhino may refresh
//! env without **`vf clr`/`vf add`** unless **`RHINO_SOURCE_FPS`** moves (cadence **`vf`** rebuild).
//! **mpv+VapourSynth** can keep a warm **Python** interpreter when **`vf`** text is unchanged; updating **`RHINO_SMOOTH_MAX_AREA`**
//! alone cannot change **`smooth_cap`** inside **`rhino_60_mvtools.vpy`**, so Rhino **forces **`vf clr`/`vf add`**
//! when the persisted ME budget differs from what the bundled graph last rebuilt with. Seek-only scrubbing never
//! schedules …
//! Clearing the graph
//! (**Smooth off** or **vf** error) restores **`hwdec=auto`** / **`vd-lavc-dr=auto`**.
//! Successful **MVTools** plugin resolution (`libmvtools.so` on Linux, `libmvtools.dylib` on
//! macOS) is stored in SQLite (`video_mvtools_lib`).
//!
//! [try_load] drains mpv so those transport events run; other hooks (speed, Preferences)
//! call [apply_mpv_video] directly. Transport **`Pause(false)`** runs [smooth_vf_attach_if_playing]
//! when **`vapoursynth`** is missing (e.g. after a seek while paused).

include!("video_pref/smooth_motion_tier.rs");
include!("video_pref/smooth_vf_me_budget_applied.rs");
include!("video_pref/mvtools_video_log_env.rs");
include!("video_pref/smooth_vf_swap_timing.rs");
include!("video_pref/mpv_escape_path.rs");
include!("video_pref/smooth_vapoursynth_vf_attach.rs");
include!("video_pref/mvtools_speed_vf_setup.rs");
include!("video_pref/smooth_off_playhead_refresh.rs");
include!("video_pref/decode_and_apply_mpv_video.rs");
include!("video_pref/smooth_me_geometry.rs");
include!("video_pref/smooth_budget.rs");
include!("video_pref/video_pref_speed_model_tests.rs");
