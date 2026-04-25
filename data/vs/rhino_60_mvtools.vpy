# Rhino Player — target ~60 fps with MVTools BlockFPS.
#
# Tuned for speed (not quality): `pel=1`, 32x32 blocks, no overlap, hex search, no DCT in analyse.
# Still CPU-heavy at 1080p; for laptops prefer lower resolution or uncheck **Smooth video (60 FPS)**.
#
# Requires: VapourSynth and the mvtools plugin, e.g.
#   Debian/Ubuntu: vapoursynth + vapoursynth-mvtools (or build from
#   https://github.com/dubhater/vapoursynth-mvtools )
#   or:  vsrepo ( https://github.com/vapoursynth/vsrepo ) and install the `mvtools` package
#
# libmpv sets global `video_in` (see mpv man vf=vapoursynth).

import vapoursynth as vs

core = vs.core
clip = video_in

sup = core.mv.Super(clip, pel=1, levels=0, chroma=True)
# search=2: 2-hex; dct=0: faster than default DCT mode for MV
bvec = core.mv.Analyse(
    sup,
    isb=True,
    blksize=32,
    overlap=0,
    search=2,
    dct=0,
)
fvec = core.mv.Analyse(
    sup,
    isb=False,
    blksize=32,
    overlap=0,
    search=2,
    dct=0,
)
out = core.mv.BlockFPS(
    clip,
    sup,
    bvec,
    fvec,
    num=60,
    den=1,
    mode=0,
    thscd=500,
    blend=True,
)
# See rhino_60_mvtools_multicore.vpy — timebase for mpv _Duration* / libmpv.
out = core.std.AssumeFPS(clip=out, fpsnum=60, fpsden=1)
out.set_output()
