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
//! can sync the **Smooth Video (~60 FPS at 1.0×)** menu.
//!
//! When attaching Smooth `vf` with media open, Rhino leaves **`hwdec`** / **`vd-lavc-dr`** unchanged
//! (usually **`hwdec=auto`**); that works on typical stacks without forcing software decode.
//! **`buffered-frames=`** in the mpv `vf vapoursynth:` string is a fixed queue depth; **`mv.Super` /
//! `mv.Analyse` / `mv.FlowFPS`** tunables live in the bundled `.vpy`, chosen from **`video_in`** short edge
//! (**≤1440** vs **>1440** tiers — see `data/vs/rhino_60_mvtools.vpy`).
//! After mpv loads a file with Smooth on at ~1.0×, the transport layer schedules [apply_mpv_video]
//! when **`FileLoaded`** or **`path`** fires (transport coalesced idle). If the active **`vf`** chain
//! already matches the resolved script and buffer settings, Rhino refreshes env vars only and skips
//! **`vf clr`**/**`vf add`** unless **[paths::RHINO_SOURCE_FPS_VAR]** changed — then it rebuilds so the
//! `.vpy` sees the new cadence (env alone does not re-init VapourSynth). Seek-only scrubbing never
//! schedules this path.
//! Clearing the graph
//! (**Smooth off** or **vf** error) restores **`hwdec=auto`** / **`vd-lavc-dr=auto`**.
//! Successful **MVTools** plugin resolution (`libmvtools.so` on Linux, `libmvtools.dylib` on
//! macOS) is stored in SQLite (`video_mvtools_lib`); the next session
//! reuses that path if the file still exists, avoiding a full search.
//!
//! [try_load] drains mpv so those transport events run; other hooks (speed, Preferences)
//! call [apply_mpv_video] directly. Transport **`Pause(false)`** runs [smooth_vf_attach_if_playing]
//! when **`vapoursynth`** is missing (e.g. after a seek while paused).

include!("video_pref/smooth_motion_tier.rs");
include!("video_pref/mvtools_video_log_env.rs");
include!("video_pref/mvtools_speed_vf_setup.rs");
include!("video_pref/decode_and_apply_mpv_video.rs");
include!("video_pref/video_pref_speed_model_tests.rs");
