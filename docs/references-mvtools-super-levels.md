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

## Rhino’s bundled preset (why **HD** uses **4**, **UHD** uses **3**)

In **`data/vs/rhino_60_mvtools.vpy`**, **`tier=uhd`** (pixel area **≥ 2560×1440**) uses **`Super(levels=3)`** to **reduce CPU/memory pressure** on very large frames— **`levels=4`** there has been seen to **stall real-time playback** on typical hardware.

**`tier=hd`** keeps **`Super(levels=4)`** as a **quality-first** default at resolutions where cost is more manageable and a deeper pyramid often matches subjective expectations for smooth interpolation.

That split is **product tuning**, not an MVTools rule.

## Further reading

- vapoursynth-mvtools **`readme.rst`** (filter signatures and **`levels=0`** default).
- Legacy MVTools / **MSuper** explanations on the Avisynth side (same hierarchical idea): e.g. [MVTools2 — MSuper](http://www.avisynth.nl/index.php/MVTools2/MSuper).

Related feature doc: [features/26-sixty-fps-motion.md](features/26-sixty-fps-motion.md).
