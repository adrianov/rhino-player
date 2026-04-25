# Smooth video playback (display / interpolation)

**Name:** Judder-free presentation on fixed-Hz displays (including 60 Hz and higher)

**Implementation status:** Done (defaults in `src/mpv_embed.rs`; no in-app UI yet)

**Use cases:** Watch 24/25/30 fps material on 60+ Hz panels without 3:2/2:2 pulldown **judder**; keep motion **smooth and stable** on typical desktop monitors. Optional future work: user toggle, per-display tuning, and lower-power / laptop profiles.

**Short description:** libmpv is configured with `video-sync=display-resample`, `interpolation` on, and `tscale=oversample` so the player **re-times decoded frames to the display refresh** (with light temporal scaling) instead of only syncing to audio with uneven frame display intervals. This is **not** a generative “soap opera” frame interpolator: it does not synthesize in-between *content* frames; it **maps existing frames in time** to the vsync (see [mpv: interpolation] and [mpv: video-sync]).

**Long description:** Many films and shows are 23.976/24/25/30 fps while monitors run 60, 100, 120+ Hz. Without re-timing, the player can hold some frames 2 display refreshes and others 3, which reads as stutter. **Display-resample** mode adjusts playback speed minutely to align frame cadence to the **actual display interval** (as reported by the VO) so each source frame is shown for a consistent, judder-mapped time. **Interpolation** turns on the interpolation filter path with **`tscale`** (temporal resampler); **`oversample`** is a good default: stable and widely used; alternatives such as `mitchell` or `catmull_rom` can trade sharpness/ringing vs smoothness. Optional **`interpolation-threshold`**, **tscale-**`window`/`blur`/`clamped`**, and **video-sync-**`max-*`** knobs exist for edge cases. **VRR (FreeSync / G-Sync)**, full-screen **exclusive** modes, and **battery/thermal** impact are environment-dependent—worth documenting in [Preferences](14-preferences.md) if we expose a toggle. **Mosaic / tiled displays** and **headless** or **very old GPUs** may need fallbacks (e.g. `video-sync=audio`).

**Specification:**

- On player init: set **`video-sync=display-resample`**, **`interpolation=yes`**, **`tscale=oversample`**. Re-assert on the open handle (same pattern as other critical options) so they apply reliably.
- **Default is on;** no new notifications. User-visible off switch is **not** in this iteration (deferred to [Preferences](14-preferences.md) and/or a future “Performance” control).
- Document here that **higher display refresh (≥60 Hz)** is where the benefit is clearest; 24p→60 is the classic case.

**Research / possible improvements (not implemented):**

| Area | Ideas |
|------|--------|
| User control | Settings key: `smooth_playback` or split `video_sync` / `interpolation` / `tscale` for advanced users. |
| tscale | Try `mitchell`, `catmull_rom`, or `lanczos` for sharpness/ringing tradeoffs; `oversample` remains the safe default. |
| VRR / vsync | On variable refresh displays, `display-resample` may interact with compositor; consider documenting “disable VRR for testing” or reading `display-fps` behavior. |
| Power | Interpolation + resample can increase GPU load slightly; a “battery saver” profile could force `video-sync=audio` and `interpolation=no`. |
| 24/1.001 vs strict 24 | Handled by mpv; if users report pull-up oddities, consider documenting `container-fps-override` or file-specific issues. |
| Thumbnails / `vo=image` | Secondary instances should not require these; keep defaults scoped to the main `Mpv` with `libmpv` VO. |

**See also:** [mpv embed](03-mpv-embedding.md), [Video options (future)](10-video-options.md), [Preferences](14-preferences.md).

[mpv: interpolation]: https://mpv.io/manual/master/#options-interpolation
[mpv: video-sync]: https://mpv.io/manual/master/#options-video-sync
