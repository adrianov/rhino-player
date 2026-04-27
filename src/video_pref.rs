//! Optional mpv VapourSynth `vf` from [crate::db::VideoPrefs].
//! See `docs/features/26-sixty-fps-motion.md`. Sets [crate::paths::RHINO_PLAYBACK_SPEED_VAR] from mpv
//! `speed` before the VapourSynth filter is built. The graph is **rebuilt on events**: after [loadfile]
//! (idle + follow-up idle), when the user picks **playback speed** in the header (deferred idle), and on
//! the **on-file-loaded** hook (one-shot) so **watch-later** restored `speed` / UI list align with the [vf]
//! and env. There is **no** periodic "watch" on `vf` for runtime plugin failures — `vf` add failure still
//! clears the pref at apply time; a script that dies *after* add is a rare install issue (toggle off in
//! **Preferences** or fix mvtools).
//! Set `RHINO_VIDEO_LOG=1` for per-step mpv result lines on stderr.
//!
//! If the VapourSynth `vf` cannot be added (no script, or mpv reports error — missing filter, plugin,
//! Python), [apply_mpv_video] sets `smooth_60` to `false`, saves settings, and returns `true` so the UI
//! can sync the **Smooth Video (~60 FPS at 1.0×)** menu.
//!
//! **Hardware decode** (`hwdec=auto` / VAAPI / NVDEC) often **bypasses** the CPU VapourSynth path, so
//! the filter is inert and motion looks identical to 24p. Normal playback leaves mpv defaults alone.
//! We set **`hwdec=no`** only while adding the mvtools [vf] (Smooth 60 and **1.0×**), and restore
//! **`hwdec=auto`** only when removing a previously active VapourSynth graph.
//! Successful `libmvtools.so` resolution is stored in SQLite (`video_mvtools_lib`); the next session
//! reuses that path if the file still exists, avoiding a full search.
//!
//! [try_load] runs [apply_mpv_video] on the next idle after [loadfile] so the `vapoursynth` [vf] is
//! installed in the same pass as the rest of the mpv state. After a **vf** clear/replace while playing,
//! we **re-seek** to the current [time-pos] so the video track realigns to audio (toggling filters or
//! sw/hw decode can leave mpv A/V offset until a seek). A **brief** black may appear while VapourSynth
//! warms up.

include!("video_pref/mvtools_speed_vf_setup.rs");
include!("video_pref/decode_and_apply_mpv_video.rs");
