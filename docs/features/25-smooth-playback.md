# Smooth video playback (display / interpolation) — removed

**Name:** Judder-free presentation on fixed-Hz displays (including 60 Hz and higher) via **mpv** `display-resample` and **interpolation** (not VapourSynth)

**Implementation status:** **Removed** from Rhino Player (OSS UI simplification, 2026). The app no longer exposes **`video-sync=display-resample`**, **`interpolation`**, or **`tscale`**. Legacy SQLite key `video_mpv_smooth` is **not** read or written.

**What replaced it:** A single checkable item **Preferences → Smooth video (60 FPS)** controls **VapourSynth**-based ~60 fps motion only; see [26-sixty-fps-motion](26-sixty-fps-motion.md). Playback always uses `video-sync=audio` and `interpolation=no` in [apply in code](../../src/video_pref.rs).

**Historical note:** When this feature *was* in the app, it re-timed decoded frames to the display refresh without synthesizing in-between *content* frames. Research ideas (tscale variants, VRR) below are **not** implemented and **not** on the current roadmap for this product shape.

| Area | Ideas (not implemented) |
|------|--------|
| tscale | `mitchell`, `catmull_rom`, or `lanczos` for sharpness/ringing tradeoffs |
| VRR / vsync | `display-fps` / compositor interactions |
| Power | battery saver profile forcing defaults off |

**See also:** [~60 fps motion](26-sixty-fps-motion.md) (active), [mpv: interpolation](https://mpv.io/manual/master/#options-interpolation), [mpv: video-sync](https://mpv.io/manual/master/#options-video-sync).
