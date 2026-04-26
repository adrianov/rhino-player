# Rhino Player — single ~60 fps preset: **FlowFPS** (lighter than old BlockFPS+Recalc) with
# **all CPU threads** and a **moderate** frame cache for mpv `concurrent-frames=auto`.
# Loads `libmvtools`: Rhino sets `RHINO_MVTOOLS_LIB` when it finds the .so, else paths below.
#
# libmpv provides global `video_in` (see mpv vf=vapoursynth). Requires: mvtools, VS R55+.
# **mpv from a shell** does not set `RHINO_MVTOOLS_LIB` (the Rhino app does). This script also
# searches `~/.local` for `libmvtools.so` (vsrepo / pipx), same idea as `src/paths.rs` in Rhino.
import os
from collections import deque

import vapoursynth as vs

_MVTOOLS_SO = "libmvtools.so"
_LOCAL_MAX_DEPTH = 14
_LOCAL_MAX_DIR_READS = 8000


def _mvtools_paths():
    p = (os.environ.get("RHINO_MVTOOLS_LIB") or "").strip()
    if p:
        yield os.path.expanduser(p)
    for c in (
        "/usr/lib/x86_64-linux-gnu/vapoursynth/libmvtools.so",
        "/usr/lib/vapoursynth/libmvtools.so",
        "/usr/local/lib/vapoursynth/libmvtools.so",
    ):
        yield c


def _mvtools_in_pipx_venvs():
    # ~/.local/share/pipx/venvs/<venv>/lib/python*/site-packages/vapoursynth/plugins/vsrepo/
    # (before scanning all of ~/.local)
    home = os.environ.get("HOME")
    if not home:
        return None
    root = os.path.join(home, ".local", "share", "pipx", "venvs")
    if not os.path.isdir(root):
        return None
    try:
        venvs = os.listdir(root)
    except OSError:
        return None
    for name in venvs:
        lib = os.path.join(root, name, "lib")
        if not os.path.isdir(lib):
            continue
        try:
            pys = os.listdir(lib)
        except OSError:
            continue
        for py in pys:
            if not py.startswith("python"):
                continue
            p = os.path.join(
                lib,
                py,
                "site-packages",
                "vapoursynth",
                "plugins",
                "vsrepo",
                _MVTOOLS_SO,
            )
            if os.path.isfile(p):
                return p
    return None


def _find_mvtools_under_local():
    home = os.environ.get("HOME")
    if not home:
        return None
    root = os.path.join(home, ".local")
    if not os.path.isdir(root):
        return None
    q = deque([(root, 0)])
    reads = 0
    while q:
        if reads >= _LOCAL_MAX_DIR_READS:
            return None
        dirpath, depth = q.popleft()
        reads += 1
        try:
            with os.scandir(dirpath) as it:
                for e in it:
                    p = e.path
                    if e.is_dir(follow_symlinks=False):
                        if depth < _LOCAL_MAX_DEPTH:
                            q.append((p, depth + 1))
                    elif e.name == _MVTOOLS_SO:
                        return p
        except OSError:
            continue
    return None


def _load_mvtools(core):
    for path in _mvtools_paths():
        if path and os.path.isfile(path):
            core.std.LoadPlugin(path=path)
            return
    pipx = _mvtools_in_pipx_venvs()
    if pipx and os.path.isfile(pipx):
        core.std.LoadPlugin(path=pipx)
        return
    extra = _find_mvtools_under_local()
    if extra and os.path.isfile(extra):
        core.std.LoadPlugin(path=extra)
        return


core = vs.core
_load_mvtools(core)
if not hasattr(core, "mv"):
    raise RuntimeError(
        "mvtools plugin not loaded (no `core.mv`). Install vapoursynth-mvtools or vsrepo, "
        "or set RHINO_MVTOOLS_LIB to the full path of libmvtools.so. "
        "From a shell:  export RHINO_MVTOOLS_LIB=/path/to/libmvtools.so"
    )
core.num_threads = os.cpu_count() or 1
# mpv can decode ahead; 8192 MB was the old “multicore” max — keep RAM use down on laptops.
core.max_cache_size = 4096

clip = video_in
src_num = clip.fps_num if clip.fps_num > 0 else 24000
src_den = clip.fps_den if clip.fps_den > 0 else 1001
clip = core.std.AssumeFPS(clip, fpsnum=src_num, fpsden=src_den)

sup = core.mv.Super(clip, pel=1, sharp=1, chroma=True)
v_backward = core.mv.Analyse(sup, blksize=32, overlap=0, search=0, isb=True)
v_forward = core.mv.Analyse(sup, blksize=32, overlap=0, search=0, isb=False)
clip = core.mv.FlowFPS(clip, sup, v_backward, v_forward, num=60, den=1, mask=0)

clip.set_output()
