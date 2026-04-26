# ~60 fps motion (optional, VapourSynth)

**Name:** Optional path to about **60 fps** display cadence for film-like sources using **synthesized in-between** frames (not **mpv** display-resample; that path is **removed** — see [25-smooth-playback](25-smooth-playback.md)).

**Implementation status:** Done (SQLite `video_smooth_60` + `video_vs_path`; main **Video** → checkable **Smooth video (60 FPS)**; bundled [`data/vs/rhino_60_mvtools_multicore.vpy`](../data/vs/rhino_60_mvtools_multicore.vpy) when `video_vs_path` is empty and the toggle is on (fallback: [`rhino_60_mvtools.vpy`](../data/vs/rhino_60_mvtools.vpy)); custom `.vpy` path optional; mvtools + VapourSynth + mpv VS build required).

**Use cases:** Viewers who want more **temporal** smoothness (often called “soap opera” or HFR) on a **~60 Hz** display.

**Short description:** A user-supplied [`.vpy` script](https://vapoursynth.com/doc) using `core.std` and plugins from [vsrepo] (e.g. **MVTools** for motion, or **RIFE-ncnn** / VSGAN for ML quality). The app ships a **MVTools BlockFPS** baseline (multicore and fast fallbacks) when no custom path is set.

**Long description:** The **VapourSynth** community uses filters such as [MVTools] block motion, [RIFE] / ncnn, or DAIN exports for fluid motion; there is no single “official” 60p preset—**quality, speed, and GPU use depend on the script and plugins**. The **mpv** [vapoursynth] filter runs a **Python** script with a global `video_in` clip; a separate mpv build may omit the filter, so the app only adds the vf when the feature exists (otherwise stderr notes). **Integrated** graphics: VapourSynth + ncnn/ML filters may use the GPU in some builds, but **MVTools** is **CPU**-oriented; lower resolution or a stronger CPU helps. If the database still has legacy **`video_frame60`** and no `video_smooth_60`, load migrates: **`vs` →** toggle **on**, **`off` →** toggle **off** (values like old `lavfi` normalize like before).

**Specification:**

- **Settings** (SQLite `settings`): `video_smooth_60` = `0` / `1`; `video_vs_path` = UTF-8 path to `.vpy` (may be empty for bundled script). [save/load](14-preferences.md) with other video prefs.
- **Menu:** **Video → Smooth video (60 FPS)** (stateful bool GAction `smooth-60`); **Choose VapourSynth script** sets the path, turns the toggle on, and saves. With no player, only prefs + menu state update; with a player, `vf` is reapplied.
- **mpv:** `video-sync=audio`, `interpolation=no`; add `vapoursynth:file=…:buffered-frames=8:concurrent-frames=auto` when the toggle is on. While 60p is **on**, **`hwdec` is forced to `no`** and **`vd-lavc-dr` to `no`** (direct rendering can also bypass the CPU filter path). The default `hwdec=auto` (VAAPI, NVDEC, etc.) often **skips the CPU VapourSynth path** — the classic “it does nothing” report. When 60p is off or the vf is rejected, **`hwdec` / `vd-lavc-dr` are set back to `auto`**. **Reapply after [loadfile]:** the filter must be added only when a media [path] is set; init-time or pre-load [vf] add is **deferred** (same tick as [try_load] `idle` + 600ms retry) so a failed early add no longer flips `video_smooth_60` to `0` in the DB. Same parallel `getFrameAsync` notes; `vd-lavc-threads=0` (auto) at init. If the `vapoursynth` **vf** cannot be added, the app sets **`video_smooth_60` to `0`**, saves, and unchecks the menu (see `data/vs/README.md`, `scripts/ensure-vapoursynth-debian.sh`). If [vf] `add` fails, the code tries [set_property] `vf` as a fallback.
- **A/B testing:** **Subtitles** (libass) are not inside the VapourSynth graph; a scrolling line can look similar at 24p vs 60p. Check **video motion** (pans) to judge HFR, or turn subs off for a second.
- **Default** when the DB has no relevant keys: toggle **on** (bundled **multicore** [BlockFPS] script, with **fast** `rhino_60_mvtools.vpy` as a documented fallback). 1080p is still **CPU**-bound.

**See also:** [25-smooth-playback](25-smooth-playback.md) (removed), [VapourSynth](https://www.vapoursynth.com/), [MVTools](https://github.com/dubhater/vapoursynth-mvtools), [RIFE](https://github.com/HolyWu/vs-rife).

[mpv]: https://mpv.io/
[vapoursynth]: https://mpv.io/manual/master/#video-filters-vapoursynth
[vsrepo]: https://github.com/vapoursynth/vsrepo
[MVTools]: https://github.com/dubhater/vapoursynth-mvtools
[RIFE]: https://github.com/HolyWu/vs-rife
