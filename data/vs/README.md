# VapourSynth (`data/vs/`)

## If 60p looks the same as 24p (VapourSynth “does nothing”)

Many systems use **hardware decode** (`hwdec=auto` → VAAPI / NVDEC / etc.). The decoded surface often **never passes through the CPU** `vapoursynth` filter, so you still see the original cadence. **Rhino** forces **`hwdec=no`** and **`vd-lavc-dr=no`** only **while the VapourSynth [vf] is actually active** (when **Preferences** → **Smooth video (~60 FPS at 1.0×)** is on **and** [mpv] **speed** is ~**1.0×**), so decoded frames are suitable for the CPU path. The **vf** is re-applied from [try_load] (idle + delayed), not only on GL init, so a filter is not installed before [path] exists. Expect higher **CPU** use; 1080p may not be real-time on weak hardware.

**Note:** On-screen **subtitles** are drawn *after* the video frame, not *through* the `.vpy` script, so a scrolling sub test can mislead; check **image** motion (pans) instead.

## Why `vf add vapoursynth` fails (`Raw(-12)`)

Rhino uses the **system** `libmpv` (see `ldd` on the binary). The error is **not** the `.vpy` file first: many Linux distros ship **mpv** / **libmpv2** built **without** the `vapoursynth` **video** filter, so that filter name never appears in `mpv -vf help` and the client API cannot add it.

- **Check:** `mpv -vf help 2>&1 | grep -E '^\s+vapoursynth\b'` — if there is **no** line, you must **replace** `libmpv`+`mpv` with a build that has VapourSynth (see `../scripts/ensure-vapoursynth-debian.sh`), or [build mpv](https://github.com/mpv-player/mpv) with `-Dvapoursynth=enabled` after VapourSynth is installed.
- **Then** install the [mvtools] plugin (`.so` for VapourSynth) so `core.mv` works in the bundled script.

## Bundled script (default 60 fps mode)

When **Preferences → Smooth video (~60 FPS at 1.0×)** is on, [mpv] **speed** is **~1.0×**, and the DB has **no** custom **Choose VapourSynth script** path, the app uses **`rhino_60_mvtools.vpy`**: it tags source fps, applies **`AssumeFPS(× speed)`** only when **speed** is not **1.0** (at **1.0** it uses the same single tag + **FlowFPS** as the old preset, to avoid A/V offset), using **`RHINO_PLAYBACK_SPEED`** (from [mpv] `speed`, set by Rhino when the vf is added or after you change **playback speed** in the UI; default `1.0` if missing), then [MVTools] **FlowFPS** to **60/1** so interpolation tracks **(source fps × speed)** (fewer in-between frames at higher speeds, or mvtools skipped when that product is already ≥ ~60). **`core.num_threads`** = all CPUs in the process affinity, **`max_cache_size` = 4096** MB, `LoadPlugin` for `libmvtools.so`, `pel=1` `Super` + `Analyse` (single pass, `overlap=0`, `search=0`) + `FlowFPS` with `mask=0`, `chroma=True` on `Super`. Rhino leaves normal playback on mpv timing defaults; the script, not mpv display-resample, creates the ~60 fps cadence. The app locates `libmvtools.so` (order: **`RHINO_MVTOOLS_LIB`**, then a path **cached in SQLite** (`video_mvtools_lib`) if that file still exists—**no full rescan**—else Debian-style paths, pipx/vsrepo under `~/.local`, then a bounded search of the rest of `~/.local`), sets **`RHINO_MVTOOLS_LIB`**, and prints **`libmvtools -> <path>`** to stderr. **Override** with `export RHINO_MVTOOLS_LIB=/path/to/libmvtools.so`. For CLI `mpv` without Rhino, you can set **`export RHINO_PLAYBACK_SPEED=1.0`** (or `1.5`, `2.0`, …) to match.

- **VapourSynth** + the **mvtools** plugin must be installed so `core.mv.*` works.
- **mpv** (the same one linked as **libmpv** for this app) must include the `vapoursynth` [vf] (`mpv -vf help`).

**Debian / Ubuntu**
- `scripts/ensure-vapoursynth-debian.sh` — checks `mpv -vf help` for the native `vapoursynth` line; optionally adds **ppa:savoury1/mpv**; installs **VapourSynth** + **MVTools** when apt can. Savoury’s **mpv 0.41** can fail dependency resolution (e.g. `libplacebo360`, FFmpeg 7) on **24.04** if parts of the stack live in private PPAs.
- `scripts/build-mpv-vapoursynth-system.sh` — **builds mpv + libmpv** from **Git** to `/usr/local` with **`-Dvapoursynth=enabled`**, using `apt build-dep mpv` + `libvapoursynth-dev`. Defaults to **mpv v0.38.0** so it matches **VapourSynth R55** from **savoury1**; **v0.39+** needs **VS R56+** (`MPV_VERSION=v0.39.0` only after upgrading VS). On **24.04+** the script sets **`Types: deb deb-src`** in `ubuntu.sources` (backups go to `/var/tmp`, not under `sources.list.d`). Set `LD_LIBRARY_PATH` for **Rhino**; **rebuild** the app is not required.

If **vf** add still fails **while mvtools should be active** (toggle on, speed ~1.0×, script path OK), Rhino turns the preference off, saves that to SQLite, and unchecks the menu so playback is not stuck retrying a broken filter. Fix the install, then turn the item on again in the menu. **Sped-up** playback (not ~1.0×) **does not** load the [vf] by design; that is not a failed add.

**Typical install (manual):** `vapoursynth` and a package that provides the `mv` plugin, e.g. `vapoursynth-mvtools`, or build [vapoursynth-mvtools] from source / use [vsrepo] (`mvtools`).

## “Extra” plugin collections (darealshinji / PPA, vsrepo)

The [darealshinji collection](https://github.com/darealshinji/vapoursynth-plugins) (and the old `vapoursynth-extra-plugins` / `ppa:djcj/vapoursynth` idea) is **archived** and **stuck on years-old** plugin versions (e.g. mvtools from that era, FFmpeg 3.x-era build glue). It is **not** a good baseline for **2024+** distros. Prefer **current** [vapoursynth-mvtools] from your distro or [vsrepo], and a **current** [VapourSynth] API (Rhino’s bundled scripts target **R55+** with **MVTools** only).

**Why Rhino does not depend on that bundle:** the bundled `.vpy` must work when the system has **only** `core.mv` (and **mpv** with `vapoursynth` vf). Pulling in **AWarpSharp2**, **BM3D**, **NNEDI3**, **waifu2x**, etc. would **fail** on machines without those `.so` files and is often **not real-time** at 1080p anyway.

**If you install more plugins yourself** (see [vsrepo]), you can use **Preferences** → **Choose VapourSynth script** with a **custom** `.vpy** that chains e.g. a **mild** spatial sharpen **after** interpolation (some people use **awarpsharp2**-style filters to counter softness from BlockFPS) or a **light** denoise **before** `Super` to stabilize vectors — at the cost of more CPU and tuning per source. We do **not** ship such scripts: they are **content- and install-specific**.

**Stronger** “looks like 60p” than MVTools is usually **separate** ML filters (e.g. **RIFE** / ncnn) — not from that static list, and not bundled here; see `docs/features/26-sixty-fps-motion.md`.

## `mpv` from the command line (not Rhino)

Use an absolute path to the script (or `$HOME/...`); the filter syntax is
`vapoursynth:file=/path/to/rhino_60_mvtools.vpy:buffered-frames=24` (or `…` in `--vf=…` with quoting as needed). The bundled script searches the same way (env, `/usr/…/vapoursynth/`, then pipx/vsrepo path under `~/.local`, then broader `~/.local`) to **`LoadPlugin`**, matching the Rhino app. If mvtools is only on your system under a path we do not search, set:

```bash
export RHINO_MVTOOLS_LIB=/full/path/to/libmvtools.so
mpv --vf=append=vapoursynth:file=\"$HOME/rhino-player/data/vs/rhino_60_mvtools.vpy\":buffered-frames=24 The.File.mkv
```

(Adjust the `file=` path; escape colons in paths if you use the `file=/a:/b` style on Windows — on Linux, prefer `file=[/path with spaces/…]`.)

## Your own script

You can use **main menu → Preferences → Choose VapourSynth script (.vpy)…**; the path is stored in the database. The [mpv] VapourSynth filter injects a global **`video_in`**; end with `…set_output()` on the last node (see a bundled script).

[mpv]: https://mpv.io/manual/master/#video-filters-vapoursynth
[mvtools]: https://github.com/dubhater/vapoursynth-mvtools
[vapoursynth-mvtools]: https://github.com/dubhater/vapoursynth-mvtools
[vsrepo]: https://github.com/vapoursynth/vsrepo
[VapourSynth]: https://www.vapoursynth.com/
