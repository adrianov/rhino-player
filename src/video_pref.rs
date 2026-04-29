//! Optional mpv VapourSynth `vf` from [crate::db::VideoPrefs].
//! See `docs/features/26-sixty-fps-motion.md`. Sets [crate::paths::RHINO_PLAYBACK_SPEED_VAR] from mpv
//! `speed` before the VapourSynth filter is built. The graph is **rebuilt on events**: after [loadfile]
//! (idle + follow-up idle), when the user picks **playback speed** in the header (deferred idle), and on
//! the **on-file-loaded** hook (one-shot) so the UI speed list and the [vf] / env stay aligned across
//! `loadfile`. There is **no** periodic "watch" on `vf` for runtime plugin failures — `vf` add failure still
//! clears the pref at apply time; a script that dies *after* add is a rare install issue (toggle off in
//! **Preferences** or fix mvtools).
//! Set `RHINO_VIDEO_LOG=1` for per-step mpv result lines on stderr.
//!
//! If the VapourSynth `vf` cannot be added (no script, or mpv reports error — missing filter, plugin,
//! Python), [apply_mpv_video] sets `smooth_60` to `false`, saves settings, and returns `true` so the UI
//! can sync the **Smooth Video (~60 FPS at 1.0×)** menu.
//!
//! **Hardware decode** (`hwdec=auto`) often **bypasses** the CPU VapourSynth path, so once the
//! Smooth filter is active the graph uses **`hwdec=no`**. After [loadfile], Smooth uses a two-idle
//! ramp: `hwdec=no` **without** `vf` first (plain software decode, visible frames), then `vf add`
//! on the next idle — avoiding a combined hw→sw switch plus VapourSynth startup on one tick. Outside
//! Smooth, mpv decode defaults are unchanged until [apply_mpv_video] adjusts them.
//! Restore **`hwdec=auto`** only when removing an active VapourSynth graph (Smooth off or error).
//! Successful `libmvtools.so` resolution is stored in SQLite (`video_mvtools_lib`); the next session
//! reuses that path if the file still exists, avoiding a full search.
//!
//! [try_load] schedules [apply_mpv_fast_start_after_load] then [apply_mpv_video] on the next idle for
//! Smooth 60 (skipped while `pause=yes`). Other hooks (speed, Preferences, transport `pause` → resume) call
//! [apply_mpv_video] directly. After a **vf**
//! clear/replace while playing,
//! we **re-seek** to the current [time-pos] so the video track realigns to audio (toggling filters or
//! sw/hw decode can leave mpv A/V offset until a seek). A **brief** black may appear while VapourSynth
//! warms up.

include!("video_pref/mvtools_speed_vf_setup.rs");
include!("video_pref/decode_and_apply_mpv_video.rs");
