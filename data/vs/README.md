# VapourSynth (`data/vs/`)

## If 60p looks the same as 24p (VapourSynth “does nothing”)

First confirm **mpv** has the **`vapoursynth`** [vf] and **MVTools** loads (`RHINO_VIDEO_LOG=1` on stderr). **Rhino** does **not** override **`hwdec`** or **`vd-lavc-dr`** when Smooth Video attaches—**`hwdec=auto`** is typical. Some **GPU / driver** stacks can route decoded frames around CPU filters; if motion clearly stays at native cadence, try **`hwdec=no`** (and optionally **`vd-lavc-dr=no`**) in mpv config as a diagnostic. The **vf** is re-applied from [try_load] (one GLib idle after **loadfile**), not only on GL init, so a filter is not installed before [path] exists. Smooth **1080p** stays **CPU**-heavy; weak hardware may not hold real-time.

**Note:** On-screen **subtitles** are drawn *after* the video frame, not *through* the `.vpy` script, so a scrolling sub test can mislead; check **image** motion (pans) instead.

## When `video_in` reports `fps_num=0 / fps_den=0`

Plenty of 29.97 / 30 / 23.976 fps mp4s — phone captures, screen recordings, web exports — reach the script with **`video_in.fps_num=0`** and **`video_in.fps_den=0`** even though the container is CFR. mpv's vapoursynth video filter does not always forward the container's rate. Without a real cadence, `FlowFPS` cannot compute output count without a guess (the previous hard-coded fallback was `24000/1001`, which silently retagged a real-29.97 source as 23.976 and stretched it by **25 %** — the "many extra frames + slowed down" drift).

Rhino works around this by reading mpv's **`container-fps`** *before* attaching the filter and exporting it as **`RHINO_SOURCE_FPS`** (decimal, e.g. `29.970030`). The bundled script falls back to that env when `video_in` is `0/0`, rationalizes it with `Fraction(...).limit_denominator(1001)` (so 29.970 → 30000/1001, 23.976 → 24000/1001, 30.0 → 30/1), and runs FlowFPS as usual. The stderr log line records the **origin** so you can see which path the script took:

```
[rhino_60_mvtools] source fps_num=30000 fps_den=1001 (origin=RHINO_SOURCE_FPS) speed=1 (1.000)
```

For genuinely VFR sources mpv has no `container-fps` either, so `RHINO_SOURCE_FPS` is unset, the script logs `source fps unknown (likely VFR; no RHINO_SOURCE_FPS)` and falls back to **passthrough** — smoothing is disabled for that file but A/V stays in sync. Re-encode to **CFR** (e.g. `ffmpeg -i in.mp4 -c:v libx264 -fps_mode cfr -r 30000/1001 out.mp4`) to get smoothing back. Running `mpv` from a shell *without* Rhino can also set `export RHINO_SOURCE_FPS=29.970030` (or whichever you know the source to be) to enable smoothing manually.

## Why `vf add vapoursynth` fails (`Raw(-12)`)

Rhino uses the **system** `libmpv` (see `ldd` on the binary). The error is **not** the `.vpy` file first: many Linux distros ship **mpv** / **libmpv2** built **without** the `vapoursynth` **video** filter, so that filter name never appears in `mpv -vf help` and the client API cannot add it.

- **Check:** `mpv -vf help 2>&1 | grep vapoursynth` — if there is **no** line, replace `libmpv`+`mpv` with a build that has VapourSynth by building [mpv](https://github.com/mpv-player/mpv) with `-Dvapoursynth=enabled` after VapourSynth is installed.
- **Then** install the [mvtools] plugin (`.so` for VapourSynth) so `core.mv` works in the bundled script.

## Bundled script (default 60 fps mode)

When **Preferences → Smooth Video (60 FPS)** is on, [mpv] **speed** is **~1.0×**, and the DB has **no** custom **Choose VapourSynth Script** path, Rhino runs **`rhino_60_mvtools.vpy`** (bundled FlowFPS toward ~60): source fps tagging, **`RHINO_PLAYBACK_SPEED`** from [mpv] `speed`, MVTools **`FlowFPS`**, **`RHINO_SMOOTH_MAX_AREA`** synced from SQLite **`video_smooth_max_area`**, **`RHINO_MVTOOLS_LIB`** discovery. **Concrete tunables** — MVTools **`blksize`**, **`overlap`**, **`Super`/`Analyse`/`FlowFPS` knobs**, **`core.num_threads`**, **`max_cache_size`**, ME crop/resize — live **only** in **`data/vs/rhino_60_mvtools.vpy`** (**`overlap` < `blksize`** remains an MVTools rule). mpv **`buffered-frames=N`** matches **`SMOOTH_VF_BUFFERED_FRAMES`** (**`src/video_pref/smooth_motion_tier.rs`**). Persisted-cap overload logic: **`src/video_pref/smooth_budget.rs`**. See also [`docs/references-mvtools-super-levels.md`](../docs/references-mvtools-super-levels.md). With Smooth vf active, Rhino sets **`video-sync=display-resample`** and **`interpolation=no`**, and reports swaps after each GL draw so interpolated vf cadence can reach the screen; stripping Smooth restores **`video-sync=audio`**. The app locates the MVTools plugin (order: **`RHINO_MVTOOLS_LIB`**, then a path **cached in SQLite** (`video_mvtools_lib`) if that file still exists—**no full rescan**—else: Linux uses Debian-style `vapoursynth/` paths, pipx/vsrepo under `~/.local`, then a bounded search of the rest of `~/.local`; macOS uses Homebrew **`vapoursynth-mvtools`** (`mvtools.dylib` under `…/vapoursynth/plugins/`) and legacy **`libmvtools.dylib`** under **`$(brew --prefix)/lib`**), sets **`RHINO_MVTOOLS_LIB`**, and prints **`libmvtools -> <path>`** to stderr. **Override** MVTools with `export RHINO_MVTOOLS_LIB=/path/to/libmvtools.so` (Linux) or `…/mvtools.dylib` / `…/libmvtools.dylib` (macOS). For CLI **mpv** without Rhino, **`export RHINO_PLAYBACK_SPEED=1.0`** (or `1.5`, `2.0`, …) and **`RHINO_SMOOTH_MAX_AREA`** as **px²** should match whichever ME cap you want Rhino to imitate (SQLite **`video_smooth_max_area`** row or documented default — see **`docs/features/26-sixty-fps-motion.md` Notes**).

- **VapourSynth** + the **mvtools** plugin must be installed so `core.mv.*` works.
- **mpv** (the same one linked as **libmpv** for this app) must include the `vapoursynth` [vf] (`mpv -vf help`).

**macOS manual install**

`brew install mpv vapoursynth-mvtools` is the whole story:

```bash
brew install mpv vapoursynth-mvtools
mpv -vf help 2>&1 | grep vapoursynth
MT=$(find "$(brew --prefix vapoursynth-mvtools)/lib" \( -name mvtools.dylib -o -name libmvtools.dylib \) | head -1)
python3 -c "import vapoursynth as vs; vs.core.std.LoadPlugin('$MT'); print(vs.core.mv)"
```

`brew install vapoursynth-mvtools` (formerly `mvtools`) pulls in `vapoursynth` and installs **`mvtools.dylib`** under `$(brew --prefix vapoursynth-mvtools)/lib/python*/site-packages/vapoursynth/plugins/`; Homebrew’s `mpv` formula (0.41+) already lists VapourSynth as a build dependency so the same `libmpv` Rhino links against can run the bundled script. Both verification commands must print non-empty output. Apple Silicon Homebrew prefix is `/opt/homebrew`, Intel is `/usr/local`; Rhino searches both layouts (including legacy **`libmvtools.dylib`** under `$(brew --prefix)/lib`). To override, `export RHINO_MVTOOLS_LIB=/full/path/to/mvtools.dylib`. If a future Homebrew `mpv` revision drops VapourSynth again, `brew reinstall mpv --build-from-source` or build it yourself with `meson setup build -Dvapoursynth=enabled`.

**Debian / Ubuntu manual install**

Install VapourSynth, build tools, and `vsrepo`, then install MVTools:

```bash
sudo apt-get update
sudo apt-get install vapoursynth vapoursynth-python3 libvapoursynth-dev pipx p7zip-full git meson ninja-build pkg-config build-essential
pipx install vsrepo
pipx ensurepath
```

Open a new terminal after `pipx ensurepath`, then run:

```bash
vsrepo update
vsrepo install mvtools
```

Verify the Python module and plugin:

```bash
python3 - <<'PY'
from pathlib import Path
import vapoursynth as vs

try:
    print(vs.core.mv)
except AttributeError:
    hits = sorted(Path.home().glob(
        ".local/share/pipx/venvs/vsrepo/lib/python*/site-packages/"
        "vapoursynth/plugins/vsrepo/libmvtools.so"
    ))
    if not hits:
        raise
    vs.core.std.LoadPlugin(str(hits[0]))
    print(vs.core.mv)
PY
```

If `mpv -vf help 2>&1 | grep vapoursynth` prints no line, build and install `mpv` + `libmpv` with VapourSynth enabled:

```bash
sudo apt-get build-dep mpv
git clone --depth 1 --branch v0.38.0 https://github.com/mpv-player/mpv.git mpv-vapoursynth
cd mpv-vapoursynth
meson setup build -Dlibmpv=true -Dvapoursynth=enabled --prefix=/usr/local
meson compile -C build
sudo meson install -C build
sudo ldconfig
mpv -vf help 2>&1 | grep vapoursynth
```

Ubuntu source repositories must be enabled for `apt-get build-dep`. `v0.38.0` is a conservative baseline for older VapourSynth R55 systems; if your VapourSynth is R56 or newer, a newer mpv tag is usually fine. If Meson reports that `build/meson-private/build.dat` was generated by an old Meson version, run `meson setup build --wipe -Dlibmpv=true -Dvapoursynth=enabled --prefix=/usr/local`, then compile and install again. If Meson is installed under `~/.local` and `sudo meson install -C build` cannot import it, use `sudo env PYTHONPATH="$(python3 -m site --user-site)" "$(command -v meson)" install -C build`. Linux builds from this repo prefer `/usr/local/lib/<multiarch>` and `/usr/local/lib` through runpath, so the locally installed `libmpv` should be found without `LD_LIBRARY_PATH`; verify with `ldd /path/to/rhino-player | grep libmpv`.

If **vf** add still fails **while mvtools should be active** (toggle on, speed ~1.0×, script path OK), Rhino turns the preference off, saves that to SQLite, and unchecks the menu so playback is not stuck retrying a broken filter. Fix the install, then turn the item on again in the menu. **Sped-up** playback (not ~1.0×) **does not** load the [vf] by design; that is not a failed add.

**Typical install (manual):** `vapoursynth` and a package that provides the `mv` plugin, e.g. `vapoursynth-mvtools`, or build [vapoursynth-mvtools] from source / use [vsrepo] (`mvtools`).

## “Extra” plugin collections (darealshinji / PPA, vsrepo)

The [darealshinji collection](https://github.com/darealshinji/vapoursynth-plugins) (and the old `vapoursynth-extra-plugins` / `ppa:djcj/vapoursynth` idea) is **archived** and **stuck on years-old** plugin versions (e.g. mvtools from that era, FFmpeg 3.x-era build glue). It is **not** a good baseline for **2024+** distros. Prefer **current** [vapoursynth-mvtools] from your distro or [vsrepo], and a **current** [VapourSynth] API (Rhino’s bundled scripts target **R55+** with **MVTools** only).

**Why Rhino does not depend on that bundle:** the bundled `.vpy` must work when the system has **only** `core.mv` (and **mpv** with `vapoursynth` vf). Pulling in **AWarpSharp2**, **BM3D**, **NNEDI3**, **waifu2x**, etc. would **fail** on machines without those `.so` files and is often **not real-time** at 1080p anyway.

**If you install more plugins yourself** (see [vsrepo]), you can use **Preferences** → **Choose VapourSynth Script** with a **custom** `.vpy** that chains e.g. a **mild** spatial sharpen **after** interpolation (some people use **awarpsharp2**-style filters to counter softness from BlockFPS) or a **light** denoise **before** `Super` to stabilize vectors — at the cost of more CPU and tuning per source. We do **not** ship such scripts: they are **content- and install-specific**.

**Stronger** “looks like 60p” than MVTools is usually **separate** ML filters (e.g. **RIFE** / ncnn) — not from that static list, and not bundled here; see `docs/features/26-sixty-fps-motion.md`.

## `mpv` from the command line (not Rhino)

Use an absolute path to the script (or `$HOME/…`). **mpv** needs `vapoursynth:file=…:buffered-frames=N:concurrent-frames=auto`; set **N** from **`SMOOTH_VF_BUFFERED_FRAMES`** in **`src/video_pref/smooth_motion_tier.rs`** (same as the app). The bundled script resolves **`LoadPlugin`** the same way as Rhino (env, distro `vapoursynth/` paths, pipx vsrepo under `~/.local`, bounded **`~/.local`** walk). From a clone (`RHINO_REPO`):

```bash
export RHINO_REPO="$HOME/rhino-player"
BF=$(sed -n 's/.*SMOOTH_VF_BUFFERED_FRAMES:.*=[[:space:]]*\([0-9][0-9]*\).*/\1/p' "$RHINO_REPO/src/video_pref/smooth_motion_tier.rs" | head -1)
export RHINO_MVTOOLS_LIB=/full/path/to/libmvtools.so
mpv --vf=append=vapoursynth:file="$RHINO_REPO/data/vs/rhino_60_mvtools.vpy":buffered-frames=${BF}:concurrent-frames=auto The.File.mkv
```

macOS (**Homebrew** dylib):

```bash
export RHINO_REPO="$HOME/rhino-player"
BF=$(sed -n 's/.*SMOOTH_VF_BUFFERED_FRAMES:.*=[[:space:]]*\([0-9][0-9]*\).*/\1/p' "$RHINO_REPO/src/video_pref/smooth_motion_tier.rs" | head -1)
export RHINO_MVTOOLS_LIB="$(find "$(brew --prefix vapoursynth-mvtools)/lib" \( -name mvtools.dylib -o -name libmvtools.dylib \) | head -1)"
mpv --vf=append=vapoursynth:file="$RHINO_REPO/data/vs/rhino_60_mvtools.vpy":buffered-frames=${BF}:concurrent-frames=auto The.File.mkv
```

(Adjust **`RHINO_REPO`** and shell quoting; prefer `file=[/path with spaces]` when needed.)


## Your own script

You can use **main menu → Preferences → Choose VapourSynth Script (.vpy)…**; the path is stored in the database. The [mpv] VapourSynth filter injects a global **`video_in`**; end with `…set_output()` on the last node (see a bundled script).

[mpv]: https://mpv.io/manual/master/#video-filters-vapoursynth
[mvtools]: https://github.com/dubhater/vapoursynth-mvtools
[vapoursynth-mvtools]: https://github.com/dubhater/vapoursynth-mvtools
[vsrepo]: https://github.com/vapoursynth/vsrepo
[VapourSynth]: https://www.vapoursynth.com/
