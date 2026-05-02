# Rhino Player — **FlowFPS** toward ~60 fps; interpolation scales with [RHINO_PLAYBACK_SPEED].
# `AssumeFPS(× speed)` *before* Super/Analyse/FlowFPS so, e.g. at 2×, natural ~48 fps (24×2) → 60
# only needs the **missing** in-betweens (~12 of 60), not a full 24→60.
# Loads `libmvtools`: Rhino sets `RHINO_MVTOOLS_LIB` when it finds the .so, else paths below.
#
# libmpv provides global `video_in` (see mpv vf=vapoursynth). Requires: mvtools, VS R55+.
# **mpv from a shell** does not set `RHINO_PLAYBACK_SPEED` (Rhino sets it from mpv `speed` when adding vf).
import math
import os
import sys
from collections import deque
from fractions import Fraction

import vapoursynth as vs

_IS_DARWIN = sys.platform == "darwin"
_MVTOOLS_SO = "libmvtools.dylib" if _IS_DARWIN else "libmvtools.so"
_LOCAL_MAX_DEPTH = 14
_LOCAL_MAX_DIR_READS = 8000
_TARGET_NUM = 60
_TARGET_DEN = 1
_TARGET_FRAC = Fraction(_TARGET_NUM, _TARGET_DEN)
_MV_OVERLAP = 16
_MV_GLOBAL_HINT = {"global": True}


def _mv_blksize() -> int:
    try:
        b = int(os.environ.get("RHINO_MV_BLKSIZE", "32"))
    except ValueError:
        b = 32
    return max(8, min(64, b))


def _mv_overlap_for(blksize: int) -> int:
    half = max(1, blksize // 2)
    try:
        raw = int(os.environ.get("RHINO_MV_OVERLAP", str(min(_MV_OVERLAP, half))))
    except ValueError:
        raw = min(_MV_OVERLAP, half)
    return max(0, min(half, raw))


def _mv_chroma() -> bool:
    v = (os.environ.get("RHINO_MV_CHROMA", "1") or "1").strip().lower()
    return v not in ("0", "false", "no", "off")

# mpv may run this script multiple times per single `vf` attach; Rhino sets `RHINO_VPY_LOG_EPOCH`
# for within-interpreter dedupe only (same epoch twice in one Python process → one stderr line).
# mpv may use a fresh interpreter after seek — the banner can repeat there; no temp files.
_logged_stderr_epoch = None


def _stderr_banner_once_per_rhino_attach(msg: str) -> bool:
    """Return True the first time (this interpreter, per Rhino vf epoch) we print `msg` to stderr."""
    global _logged_stderr_epoch
    epoch = (os.environ.get("RHINO_VPY_LOG_EPOCH") or "").strip()
    if epoch:
        if _logged_stderr_epoch == epoch:
            return False
        _logged_stderr_epoch = epoch
    print(msg, file=sys.stderr, flush=True)
    return True

def _mvtools_paths():
    p = (os.environ.get("RHINO_MVTOOLS_LIB") or "").strip()
    if p:
        yield os.path.expanduser(p)
    if _IS_DARWIN:
        # `brew install mvtools` drops libmvtools.dylib under $(brew --prefix)/lib.
        for c in (
            "/opt/homebrew/lib/libmvtools.dylib",
            "/usr/local/lib/libmvtools.dylib",
        ):
            yield c
        return
    for c in (
        "/usr/lib/x86_64-linux-gnu/vapoursynth/libmvtools.so",
        "/usr/lib/vapoursynth/libmvtools.so",
        "/usr/local/lib/vapoursynth/libmvtools.so",
    ):
        yield c


def _mvtools_in_pipx_venvs():
    if _IS_DARWIN:
        # vsrepo / pipx is the Linux install path; on macOS Homebrew already covers everything.
        return None
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
    if _IS_DARWIN:
        return None
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


def _speed_frac_from_env() -> Fraction:
    """Exact 1, 1.5, 2 — avoids float/Fraction drift that can skew FlowFPS timebase (A/V at 1×)."""
    t = (os.environ.get("RHINO_PLAYBACK_SPEED") or "1").strip() or "1"
    if t in ("1", "1.0", "1.00"):
        return Fraction(1, 1)
    if t in ("1.5", "1.50"):
        return Fraction(3, 2)
    if t in ("2", "2.0", "2.00"):
        return Fraction(2, 1)
    try:
        x = float(t)
    except ValueError:
        return Fraction(1, 1)
    if not (0.01 < x < 8.0):
        return Fraction(1, 1)
    return Fraction(x).limit_denominator(4)


def _src_fps_from_env() -> tuple[int, int] | None:
    """Recover the source cadence from `RHINO_SOURCE_FPS` (Rhino sets it from mpv's `container-fps`).

    Many otherwise-CFR mp4s reach this script with `video_in.fps_num=0 / fps_den=0` because mpv's
    vapoursynth filter does not always forward the container's rate. Without a fallback the script
    has to give up and passthrough, which kills smoothing for plain 29.97 / 23.976 / 30 clips.
    `limit_denominator(1001)` snaps decimals back to NTSC-friendly rationals (29.970 -> 30000/1001,
    23.976 -> 24000/1001, 30.0 -> 30/1) so FlowFPS still gets an exact ratio.
    """
    t = (os.environ.get("RHINO_SOURCE_FPS") or "").strip()
    if not t:
        return None
    try:
        x = float(t)
    except ValueError:
        return None
    if not (0.1 < x < 1000.0):
        return None
    f = Fraction(x).limit_denominator(1001)
    return f.numerator, f.denominator


def _mul_src_fps_rational(num: int, den: int, speed_frac: Fraction) -> tuple[int, int]:
    a = Fraction(num, den) * speed_frac
    n, d = a.numerator, a.denominator
    g = math.gcd(n, d)
    return n // g, d // g


def _as_float(n: int, d: int) -> float:
    if d == 0:
        return 0.0
    return n / d


def _vs_threads() -> int:
    """Use every CPU in Rhino's scheduler affinity for the heavy mvtools graph."""
    try:
        n = len(os.sched_getaffinity(0))
    except (AttributeError, OSError):
        n = os.cpu_count() or 1
    return max(1, n)


core = vs.core
_load_mvtools(core)
if not hasattr(core, "mv"):
    raise RuntimeError(
        "mvtools plugin not loaded (no `core.mv`). On Linux: install vapoursynth-mvtools or "
        "vsrepo. On macOS: `brew install mvtools`. Or set RHINO_MVTOOLS_LIB to the full path of "
        f"{_MVTOOLS_SO}, e.g. export RHINO_MVTOOLS_LIB=/opt/homebrew/lib/libmvtools.dylib"
    )
core.num_threads = _vs_threads()
core.max_cache_size = 4096

clip = video_in
spd_frac = _speed_frac_from_env()
src_num = clip.fps_num
src_den = clip.fps_den
# `video_in` reports the source rate as `0/0` for **VFR** clips (no single cadence) **and**
# for many CFR mp4s where mpv's vapoursynth filter simply does not forward the container's
# fps. Rhino publishes `RHINO_SOURCE_FPS` from mpv's `container-fps` exactly to recover
# smoothing in that second case — fall back to it when present, then **only** passthrough
# when the cadence really is unknown. AssumeFPS-ing a true VFR clip to a guessed rate (the
# old fallback was hard-coded `24000/1001`) silently rescales declared duration: an actual
# 29.97 retagged as 23.976 then FlowFPS'd to 60 produces `2.5N` output frames = **25 %
# longer than audio**, the "many frames + slowed down" drift the user reported.
fps_origin = "video_in"
if src_num <= 0 or src_den <= 0:
    fb = _src_fps_from_env()
    if fb is not None:
        src_num, src_den = fb
        fps_origin = "RHINO_SOURCE_FPS"
_banner = (
    f"[rhino_60_mvtools] source fps_num={src_num} fps_den={src_den} "
    f"(origin={fps_origin}) speed={spd_frac} ({float(spd_frac):.3f})"
)
if src_num <= 0 or src_den <= 0 or _as_float(src_num, src_den) <= 0:
    if _stderr_banner_once_per_rhino_attach(_banner):
        print(
            "[rhino_60_mvtools] source fps unknown (likely VFR; no RHINO_SOURCE_FPS) — "
            "passthrough, smoothing skipped to avoid A/V drift",
            file=sys.stderr,
            flush=True,
        )
    clip.set_output()
else:
    _stderr_banner_once_per_rhino_attach(_banner)
    clip = core.std.AssumeFPS(clip, fpsnum=src_num, fpsden=src_den)
    base_src = Fraction(src_num, src_den)
    # Passthrough mvtools only when (source fps × speed) is **already** ≥ 60/1 in **rational** math.
    # Do **not** use `float` + ε here: common tags like 60000/1001 (≈59.94) plus 0.2 hit ≥60 and
    # accidentally skipped **FlowFPS** (regression: “24p” looked un-smoothed when mpv reported ~59.94).
    if base_src * spd_frac >= Fraction(60, 1):
        clip.set_output()
    else:
        # 1.0×: do **not** re-tag the clip; second AssumeFPS(×1) + FlowFPS can skew duration
        # vs audio (mvtools/mpv clock). 1.5/2.0: retime so FlowFPS only fills toward 60 from
        # (fps×S).
        if spd_frac == 1:
            work = clip
        else:
            sn, sd = _mul_src_fps_rational(src_num, src_den, spd_frac)
            work = core.std.AssumeFPS(clip, fpsnum=sn, fpsden=sd)
        # Keep the fast preset, blend block edges, let MVTools detect broad camera pans.
        _blk = _mv_blksize()
        _ov = _mv_overlap_for(_blk)
        _chroma = _mv_chroma()
        sup = core.mv.Super(work, pel=1, sharp=1, chroma=_chroma)
        v_backward = core.mv.Analyse(
            sup,
            blksize=_blk,
            overlap=_ov,
            search=0,
            isb=True,
            **_MV_GLOBAL_HINT,
        )
        v_forward = core.mv.Analyse(
            sup,
            blksize=_blk,
            overlap=_ov,
            search=0,
            isb=False,
            **_MV_GLOBAL_HINT,
        )
        clip = core.mv.FlowFPS(
            work,
            sup,
            v_backward,
            v_forward,
            num=_TARGET_NUM,
            den=_TARGET_DEN,
            mask=0,
        )
        clip.set_output()
