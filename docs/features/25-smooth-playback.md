# Smooth video playback (display / interpolation) — removed

**Name:** Judder-free presentation on fixed-Hz displays (including 60 Hz and higher) via **mpv** `display-resample` and **interpolation** (not VapourSynth)

**Implementation status:** **Default presentation path** (no user preference). Rhino no longer exposes a general smooth-playback toggle and does not read or write the legacy SQLite key `video_mpv_smooth`, but the embedded player now uses **`video-sync=display-resample`**, **`interpolation=yes`**, and **`tscale=oversample`** by default to reduce base playback pan tearing/judder.

**What replaced the old toggle:** A single checkable item **Preferences → Smooth video (~60 FPS at 1.0×)** stores intent for **VapourSynth**-based ~60 fps motion; the filter runs at **~1.0×** only; see [26-sixty-fps-motion](26-sixty-fps-motion.md). The always-on display-resample path is only for presentation cadence; the active VapourSynth path still creates ~60 fps frames in the filter graph.

**Historical note:** When this feature *was* in the app, it re-timed decoded frames to the display refresh without synthesizing in-between *content* frames. Research ideas (tscale variants, VRR) below are **not** implemented and **not** on the current roadmap for this product shape.

| Area | Ideas (not implemented) |
|------|--------|
| tscale | `mitchell`, `catmull_rom`, or `lanczos` for sharpness/ringing tradeoffs |
| VRR / vsync | `display-fps` / compositor interactions |
| Power | battery saver profile forcing defaults off |

**See also:** [~60 fps motion](26-sixty-fps-motion.md) (active), [mpv: interpolation](https://mpv.io/manual/master/#options-interpolation), [mpv: video-sync](https://mpv.io/manual/master/#options-video-sync).
