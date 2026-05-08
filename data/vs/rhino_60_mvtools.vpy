# Rhino Player — **FlowFPS** toward ~60 fps; interpolation scales with [RHINO_PLAYBACK_SPEED].
# `AssumeFPS(× speed)` *before* Super/Analyse/FlowFPS so, e.g. at 2×, natural ~48 fps (24×2) → 60
# only needs the **missing** in-betweens (~12 of 60), not a full 24→60.
# Loads `libmvtools`: Rhino sets `RHINO_MVTOOLS_LIB` when it finds the .so, else paths below.
#
# libmpv provides global `video_in` (see mpv vf=vapoursynth). Requires: mvtools, VS R55+.
# Source fps: ``RHINO_SOURCE_FPS`` when set (Rhino: mpv ``container-fps``), else ``video_in.fps_*``,
# else CFR from frame props — vf-forwarded rationals can disagree with the container (~29.97 NTSC).
# **mpv from a shell** does not set `RHINO_PLAYBACK_SPEED` (Rhino sets it from mpv `speed` when adding vf).
import math
import os
import sys
from collections import deque
from decimal import Decimal, InvalidOperation
from fractions import Fraction

import vapoursynth as vs

_IS_DARWIN = sys.platform == "darwin"
_MVTOOLS_SO = "libmvtools.dylib" if _IS_DARWIN else "libmvtools.so"
_LOCAL_MAX_DEPTH = 14
_LOCAL_MAX_DIR_READS = 8000
_TARGET_NUM = 60
_TARGET_DEN = 1
_TARGET_FRAC = Fraction(_TARGET_NUM, _TARGET_DEN)
_MV_GLOBAL_HINT = {"global": True}  # global motion predictor on ``Analyse`` (extra ME path)
# MVTools ``Analyse``: ``blksize`` / ``overlap`` match upstream defaults (**8** / **0**). ``overlap`` must be **< blksize**.
# Smaller blocks ⇒ **more** motion blocks per frame (heavier ``Analyse``, finer spatial sampling vs large-block presets).
# **Decode-area policy**: Rhino sets ``RHINO_SMOOTH_MAX_AREA`` (persisted max decode width×height for ME).
# If **decode** ``width × height`` exceeds that budget, **both** dimensions scale by ``√(budget/decode_px)``
# so MVTools runs at **≤ budget** pixels (**stderr ``path=scaled``**); otherwise **native** (**``path=full``**).
# **FlowFPS** runs on that ME raster; **set_output** uses **FlowFPS** dimensions (ME raster size — may be smaller
# than decode when ``path=scaled``). mpv scales to the window; geometry may **diverge** from ``video_in``.
# ``tier=uhd`` stderr tag when **decode** ``width × height >= _MV_DECODE_TAG_AREA`` (**2560×1440**); ``tier=hd`` otherwise (logging only).
# **Both paths**: ``chroma=True`` on ``Super`` / ``Analyse`` (luma-only ME yields bad chroma — green tint).
_MV_BLK_SIZE = 8
_MV_BLK_OVERLAP = 0
# ``Super`` **`levels`**: **0** = MVTools **automatic** pyramid depth from frame size (upstream default).
_MV_SUPER_LEVELS = 0
_MV_DECODE_TAG_AREA = 2560 * 1440
# Explicit ``Super`` edge padding (MVTools default is **16**).
_MV_SUPER_PAD = 16


def _me_wh_aligned(width_px: int, height_px: int) -> tuple[int, int]:
    """Floor to multiples of ``_MV_BLK_SIZE`` (min ``_MV_BLK_SIZE`` per side) for MVTools block grid / chroma."""
    b = _MV_BLK_SIZE
    w = max(b, width_px - (width_px % b))
    h = max(b, height_px - (height_px % b))
    return w, h


def _smooth_max_area_px() -> int:
    """ME pixel budget from Rhino (**RHINO_SMOOTH_MAX_AREA**; default **1920×1080** decode-area equivalent)."""
    raw = (os.environ.get("RHINO_SMOOTH_MAX_AREA") or "").strip()
    if not raw:
        return 1920 * 1080
    try:
        return max(1, int(raw))
    except ValueError:
        return 1920 * 1080


def _mv_graph_for_frame_size(
    width_px: int,
    height_px: int,
) -> tuple[str, int, int, int, int, int, int, bool, bool, bool]:
    """Tier tag + MVTools params.

    Returns ``(tier, blksize, overlap, pel, sharp, search, flow_mask, flow_blend,
    truemotion, chroma)``.
    ``tier`` is **uhd** when **decode** ``width × height >= _MV_DECODE_TAG_AREA``, else **hd** (stderr tag only).
    ME raster may be **proportionally** smaller than decode when ``RHINO_SMOOTH_MAX_AREA`` < decode pixels.
    ME **width** / **height** are floored to multiples of ``_MV_BLK_SIZE`` (crop right/bottom on **full** path when
    possible; else **Bicubic** — e.g. tiny frames).
    Shared ``blksize`` / ``overlap`` **8** / **0** (MVTools upstream defaults).
    ``mv.Super`` ``levels=0`` (automatic pyramid) on the MVTools raster; **FlowFPS** output keeps **ME** frame size
    (may differ from decode when ``RHINO_SMOOTH_MAX_AREA`` constrained the ME raster).
    **Both paths**: ``chroma=True`` on ``Super`` / ``Analyse``.
    Shared: ``pel=1``, ``sharp=1``, ``search=4`` (hex), ``truemotion=True``, ``FlowFPS(mask=2, blend=True)``, ``global=True``.
    """
    w = max(1, width_px)
    h = max(1, height_px)
    tier = "uhd" if w * h >= _MV_DECODE_TAG_AREA else "hd"
    blk, ov = _MV_BLK_SIZE, _MV_BLK_OVERLAP
    return (
        tier,
        blk,
        ov,
        1,
        1,
        4,
        2,
        True,  # FlowFPS blend
        True,  # Analyse truemotion
        True,
    )


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
    """Recover cadence from ``RHINO_SOURCE_FPS`` (Rhino: mpv ``container-fps``).

    Parse as ``Decimal`` → ``Fraction`` so decimals like ``29.970030`` snap via ``limit_denominator(1001)``
    to NTSC rationals (e.g. ``30000/1001``) without binary-float noise. Wrong rationals from vf forwarding
    would skew FlowFPS output-vs-input frame quotients.
    """
    t = (os.environ.get("RHINO_SOURCE_FPS") or "").strip()
    if not t:
        return None
    try:
        dec = Decimal(t)
    except InvalidOperation:
        return None
    if dec <= 0:
        return None
    x = float(dec)
    if not (0.1 < x < 1000.0):
        return None
    f = Fraction(dec).limit_denominator(1001)
    return f.numerator, f.denominator


def _int_frame_prop(props, *keys: str) -> int | None:
    for k in keys:
        try:
            v = props[k]
        except KeyError:
            continue
        try:
            iv = int(v)
            if iv > 0:
                return iv
        except (TypeError, ValueError):
            continue
    return None


def _fps_rational_from_delta_ns(delta_ns: int) -> tuple[int, int] | None:
    """delta_ns = nanoseconds between consecutive frames → fps = 1e9/delta as reduced rational."""
    if delta_ns < 1000:
        return None
    num = 1_000_000_000
    den = delta_ns
    g = math.gcd(num, den)
    num //= g
    den //= g
    fps_f = num / den if den else 0.0
    if not (0.5 <= fps_f <= 120.0):
        return None
    return num, den


def _src_fps_from_frame_props(clip: vs.VideoNode) -> tuple[int, int] | None:
    """Infer CFR from decoder frame props when ``fps_num/fps_den`` are unset.

    Some pipelines attach ``_Duration`` (nanoseconds per frame) or consecutive ``_AbsoluteTime``
    values; both imply a constant frame interval. We require frames 0 and 1 to agree so we do not
    guess VFR content. This covers **shell mpv** (no ``RHINO_SOURCE_FPS``) and cases where the host
    has not yet published ``container-fps``.
    """
    if clip.num_frames < 2:
        return None
    try:
        f0 = clip.get_frame(0)
        f1 = clip.get_frame(1)
        p0, p1 = f0.props, f1.props
        d0 = _int_frame_prop(p0, "_Duration", "Duration", "_FrameDuration")
        d1 = _int_frame_prop(p1, "_Duration", "Duration", "_FrameDuration")
        if d0 is not None and d1 is not None and d0 == d1:
            out = _fps_rational_from_delta_ns(d0)
            if out is not None:
                return out
        t0 = _int_frame_prop(p0, "_AbsoluteTime", "AbsoluteTime")
        t1 = _int_frame_prop(p1, "_AbsoluteTime", "AbsoluteTime")
        if t0 is not None and t1 is not None:
            dt = t1 - t0
            if dt > 0:
                return _fps_rational_from_delta_ns(dt)
    except Exception:
        return None
    return None


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
# Prefer ``RHINO_SOURCE_FPS`` when Rhino/mpv publishes it: vapoursynth bridge often leaves
# ``fps_num/fps_den`` at **0/0** **or** forwards a coarse rational that disagrees with the container
# (~29.97 NTSC must match ``30000/1001`` for FlowFPS frame quotient math).
fps_origin = "video_in"
fb_env = _src_fps_from_env()
if fb_env is not None:
    src_num, src_den = fb_env
    fps_origin = "RHINO_SOURCE_FPS"
else:
    src_num = clip.fps_num
    src_den = clip.fps_den
    if src_num <= 0 or src_den <= 0:
        fb = _src_fps_from_frame_props(clip)
        if fb is not None:
            src_num, src_den = fb
            fps_origin = "frame_props(CFR)"
_banner = (
    f"[rhino_60_mvtools] source fps_num={src_num} fps_den={src_den} "
    f"(origin={fps_origin}) speed={spd_frac} ({float(spd_frac):.3f})"
)
if src_num <= 0 or src_den <= 0 or _as_float(src_num, src_den) <= 0:
    if _stderr_banner_once_per_rhino_attach(_banner):
        print(
            "[rhino_60_mvtools] source fps unknown (no video_in fps, no CFR frame props, "
            "no RHINO_SOURCE_FPS) — passthrough, smoothing skipped to avoid A/V drift",
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
        _decode_w = work.width
        _decode_h = work.height
        _decode_px = _decode_w * _decode_h
        _smooth_cap = _smooth_max_area_px()
        if _decode_px <= _smooth_cap:
            _aw, _ah = _me_wh_aligned(_decode_w, _decode_h)
            if _aw == _decode_w and _ah == _decode_h:
                work_me = work
                _me_path = "full"
            elif _aw <= _decode_w and _ah <= _decode_h:
                work_me = core.std.Crop(
                    work,
                    left=0,
                    right=_decode_w - _aw,
                    top=0,
                    bottom=_decode_h - _ah,
                )
                _me_path = "full"
            else:
                work_me = core.resize.Bicubic(work, width=_aw, height=_ah)
                _me_path = "full"
        else:
            _scale = math.sqrt(float(_smooth_cap) / float(_decode_px))
            _me_w = max(2, int(round(_decode_w * _scale)))
            _me_h = max(2, int(round(_decode_h * _scale)))
            _aw, _ah = _me_wh_aligned(_me_w, _me_h)
            work_me = core.resize.Bicubic(work, width=_aw, height=_ah)
            _me_path = "scaled"
        _tier, _blk, _ov, _pel, _sharp, _search, _fmask, _fblend, _tm, _mv_chroma = (
            _mv_graph_for_frame_size(_decode_w, _decode_h)
        )
        _super_kw = {
            "pel": _pel,
            "sharp": _sharp,
            "chroma": _mv_chroma,
            "levels": _MV_SUPER_LEVELS,
            "hpad": _MV_SUPER_PAD,
            "vpad": _MV_SUPER_PAD,
        }
        _super_lvl_log = "auto" if _MV_SUPER_LEVELS == 0 else str(_MV_SUPER_LEVELS)
        print(
            f"[rhino_60_mvtools] mvtools preset tier={_tier} "
            f"(decode_px={_decode_px}, smooth_cap={_smooth_cap}) "
            f"path={_me_path} decode={_decode_w}x{_decode_h} vf_out={work_me.width}x{work_me.height} "
            f"blksize={_blk} overlap={_ov} chroma={_mv_chroma} "
            f"pel={_pel} sharp={_sharp} search={_search} global={_MV_GLOBAL_HINT['global']} "
            f"Super.levels={_super_lvl_log} hpad={_MV_SUPER_PAD} vpad={_MV_SUPER_PAD} "
            f"truemotion={_tm} FlowFPS(mask={_fmask},blend={_fblend})",
            file=sys.stderr,
            flush=True,
        )
        sup = core.mv.Super(work_me, **_super_kw)
        v_backward = core.mv.Analyse(
            sup,
            blksize=_blk,
            overlap=_ov,
            search=_search,
            chroma=_mv_chroma,
            truemotion=_tm,
            isb=True,
            **_MV_GLOBAL_HINT,
        )
        v_forward = core.mv.Analyse(
            sup,
            blksize=_blk,
            overlap=_ov,
            search=_search,
            chroma=_mv_chroma,
            truemotion=_tm,
            isb=False,
            **_MV_GLOBAL_HINT,
        )
        smooth = core.mv.FlowFPS(
            work_me,
            sup,
            v_backward,
            v_forward,
            num=_TARGET_NUM,
            den=_TARGET_DEN,
            mask=_fmask,
            blend=_fblend,
        )
        clip = smooth
        clip.set_output()
