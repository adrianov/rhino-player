//! Optional mpv VapourSynth `vf` from [crate::db::VideoPrefs].
//! See `docs/features/26-sixty-fps-motion.md`. Sets [crate::paths::RHINO_PLAYBACK_SPEED_VAR] from mpv
//! `speed` before the VapourSynth filter is built. The graph is **rebuilt on events**: after mpv
//! reports new media (**`FileLoaded`** and **`path`** change — coalesced to one idle — covering Open,
//! drag-drop, sibling EOF advance, **Previous** / **Next**, and `loadfile`), when the user picks
//! **playback speed** in the header (deferred idle), and on unpause after Smooth unloaded the `vf`
//! for a still frame. There is **no** periodic "watch" on `vf` for runtime plugin failures — `vf` add failure still
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
//! After mpv loads a file with Smooth on at ~1.0×, the transport layer schedules [apply_mpv_video]
//! when **`FileLoaded`** or **`path`** fires (transport coalesced idle). Clearing the graph
//! (**Smooth off** or **vf** error) restores **`hwdec=auto`** / **`vd-lavc-dr=auto`**.
//! Successful `libmvtools.so` resolution is stored in SQLite (`video_mvtools_lib`); the next session
//! reuses that path if the file still exists, avoiding a full search.
//!
//! [try_load] drains mpv so those transport events run; other hooks (speed, Preferences)
//! call [apply_mpv_video] directly. Transport `pause=no` re-attaches the `vf` after Smooth unloaded it for a paused still frame.

include!("video_pref/mvtools_speed_vf_setup.rs");
include!("video_pref/decode_and_apply_mpv_video.rs");
include!("video_pref/video_pref_speed_model_tests.rs");
