# Rhino Player — ~60 fps MVTools **high quality** (BlockFPS + Recalculate, pel=4).
# Heavier CPU than the old `rhino_60_mvtools.vpy` fast preset; for laptops use that one or
# Uncheck **Video → Smooth video (60 FPS)** when the machine cannot keep up (drops / fan).
#
# Tuned for mpv’s VapourSynth vf (`concurrent-frames=auto`, parallel getFrameAsync) with
# `core.num_threads` and a large `max_cache_size`.
#
# libmpv provides global `video_in` (see mpv vf=vapoursynth). Requires: mvtools, VS R55+.
import os
import vapoursynth as vs

core = vs.core
n = os.cpu_count() or 1
core.num_threads = n
core.max_cache_size = 8192

clip = video_in
dst_fps = 60

# pel=4: 1/4-pel super clip (best common trade vs FlowFPS; much heavier than pel=2).
# Wider padding so motion at borders is estimated more safely at high overlap.
sup = core.mv.Super(clip, pel=4, hpad=32, vpad=32, levels=0, chroma=True)

# First pass: small blocks, overlap, and DCT-based SAD for texture-heavy motion.
bvec0 = core.mv.Analyse(
    sup,
    isb=True,
    blksize=4,
    overlap=2,
    search=4,
    dct=1,
)
fvec0 = core.mv.Analyse(
    sup,
    isb=False,
    blksize=4,
    overlap=2,
    search=4,
    dct=1,
)
# Recalculate: refine vectors (major quality boost; second pass of motion search).
# Python: `lambda` is reserved — use _lambda (see mvtools issue #17).
bvec = core.mv.Recalculate(
    sup,
    bvec0,
    blksize=4,
    overlap=2,
    search=2,
    dct=1,
    _lambda=180,
)
fvec = core.mv.Recalculate(
    sup,
    fvec0,
    blksize=4,
    overlap=2,
    search=2,
    dct=1,
    _lambda=180,
)
# mode=3 is mvtools’ default; blend reduces warping on hard cuts vs blend=False.
out = core.mv.BlockFPS(
    clip,
    sup,
    bvec,
    fvec,
    num=dst_fps,
    den=1,
    mode=3,
    thscd1=500,
    thscd2=130,
    blend=True,
)
# mpv reads _DurationNum/_Den (vf_vapoursynth). AssumeFPS is metadata-only.
out = core.std.AssumeFPS(clip=out, fpsnum=dst_fps, fpsden=1)
out.set_output()
