# MVTools `Super`: pyramid **`levels`** (smoothness vs cost)

This note summarizes how **`mv.Super`** **`levels`** behaves in [vapoursynth-mvtools](https://github.com/dubhater/vapoursynth-mvtools) and answers whether motion interpolation **needs** a minimum depth (e.g. **4**) for acceptable smoothness.

## What **`levels`** does

**`Super`** builds a **multi-resolution pyramid** of the source frames (scaled halving steps). **`Analyse`** walks that pyramid so motion search can start coarse (large displacements, cheaper blocks) and refine toward the full resolution. **`FlowFPS`** (and similar filters) consume the motion fields **`Analyse`** produces.

In the upstream signature, **`levels`** defaults to **`0`**, which means **automatic**: compute however many pyramid levels the implementation derives from frame size (full pyramid), not “zero levels.” See the plugin readme ([`readme.rst`](https://github.com/dubhater/vapoursynth-mvtools/blob/master/readme.rst)) — **`mv.Super(..., int levels=0, ...)`**.

Setting **`levels`** to a **positive integer** **caps** how many hierarchical levels **`Super`** prepares. Fewer levels ⇒ **less work and memory per frame** (especially painful at **4K** widths), but also **less** coarse-to-fine refinement—when motion is large or complex, vectors can be worse and interpolated frames can look **less stable or less “smooth”** subjectively.

## Is **`levels ≥ 4`** required for smooth motion?

**No.** Upstream does **not** document a minimum **`levels`** for “smooth” **`FlowFPS`** output. Smoothness is **scene-dependent** and interacts with **`blksize`**, **`overlap`**, **`search`**, **`pel`**, **`truemotion`**, masking/blending on **`FlowFPS`**, and playback **fps** targets—not pyramid depth alone.

What people usually observe:

- **Higher** **`levels`** (up to what the resolution supports) tends to **help** hierarchical estimation when **large** motions must be found before refining—often subjectively **better** interpolation on hard pans or fast motion.
- **Lower** **`levels`** can still look fine on **gentler** motion or smaller frames; there is **no universal cutoff** at 3 vs 4—only workload vs quality tradeoffs.

## Low-res motion vectors on the full-resolution picture?

**`mv.FlowFPS`** expects the **same frame size** for the **input clip**, the **`Super`** clip, and the **`Analyse`** vector clips. MVTools does **not** ship a filter that recomputes **`Super`** at full resolution while reusing vectors from a smaller raster.

Third-party **[vapoursynth-manipmv](https://pypi.org/project/vapoursynth-manipmv/)** ( **`ScaleVect`** — [upstream repo](https://github.com/Mikewando/manipulate-motion-vectors)) can **scale vector clips** (block grid, overlaps, padding, motion samples) so motion can be **estimated on a proxy raster** and **`mv.FlowFPS`** run at **full decode size**. **`mv.Super`** on the full-resolution clip must use **`hpad` / `vpad`** scaled by the same factor as the vectors (see the plugin README **× SCALE** pattern).

## Rhino’s bundled preset (`rhino_60_mvtools.vpy`)

**Block size vs cost:** MVTools **`Analyse`** walks a grid of motion blocks. **Smaller `blksize`** ⇒ **more blocks per frame** at the same width×height ⇒ **`Analyse` tends to get heavier**, not lighter.

Rhino’s bundled script runs **`mv.Super`** / **`Analyse`** / **`mv.FlowFPS`** on **native decode** when **`tier=hd`**, and on a **half-resolution** **`resize.Bicubic`** copy when **`tier=uhd`** (**decode** area **≥ 2560×1440**). **UHD** leaves output at **half** size (**no** upscale). Stderr **`tier=`** reflects decode class; **`path=full`** vs **`path=half`** logs which raster MVTools used. **`Super`** / **`Analyse`** use **`chroma=true`** on **both** paths (luma-only ME breaks chroma). Same **`blksize` / `overlap`** (**`128` / `64`**); **`Super.levels=0`** (automatic pyramid — MVTools default). **`Analyse`** uses **`search=2`**, **`truemotion=true`**, **`global=true`** (**`Super`**: **`pel=1`**, **`sharp=1`**).

For experiments with **proxy motion estimation** or **vector scaling**, third-party **[vapoursynth-manipmv](https://pypi.org/project/vapoursynth-manipmv/)** (**`ScaleVect`**) remains documented below as general MVTools ecosystem reading — it is **not** wired into Rhino’s bundled preset.

## Further reading

- vapoursynth-mvtools **`readme.rst`** (filter signatures and **`levels=0`** default).
- [vapoursynth-manipmv](https://pypi.org/project/vapoursynth-manipmv/) (**`ScaleVect`**).
- Legacy MVTools / **MSuper** explanations on the Avisynth side (same hierarchical idea): e.g. [MVTools2 — MSuper](http://www.avisynth.nl/index.php/MVTools2/MSuper).

Related feature doc: [features/26-sixty-fps-motion.md](features/26-sixty-fps-motion.md).
