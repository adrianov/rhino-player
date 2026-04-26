# Icons (Freedesktop / GNOME)

## Layout

- `hicolor/<NxN>/apps/ch.rhino.RhinoPlayer.png` — [icon theme] assets; name matches [GApplication id] `ch.rhino.RhinoPlayer` and the `Icon=` key in the [`.desktop`] file. Standard sizes: **16, 22, 24, 32, 48, 64, 128, 256, 512, 1024**.
- `source/ch.rhino.RhinoPlayer-source-gemini-1024.png` — **unmodified** 1024×1024 design export (arbitrary “paper” background; archive reference).
- `source/ch.rhino.RhinoPlayer-master-1024.png` — **processed** 1024×1024: transparent background, art **trimmed** to content, **squared** to the minimal bounding square (by extent, no stretch), then **scaled to the full 1024×1024** canvas. Regenerates all hicolor sizes from this.

## Regenerating (ImageMagick)

Run **`rebuild-png-assets.sh`** from the project root (or this directory). Default first argument is the **Gemini** export; output is the master and every `hicolor/…/ch.rhino.RhinoPlayer.png`.

```bash
./data/icons/rebuild-png-assets.sh
# or explicitly:
./data/icons/rebuild-png-assets.sh data/icons/source/ch.rhino.RhinoPlayer-source-gemini-1024.png 8
# optional third: inset (0.00–0.12) to leave a small transparent margin inside 1024 (e.g. 0.04 = ~4% per side)
./data/icons/rebuild-png-assets.sh data/icons/source/ch.rhino.RhinoPlayer-source-gemini-1024.png 8 0.04
```

Arguments:

1. **Source PNG** (default: `data/icons/source/ch.rhino.RhinoPlayer-source-gemini-1024.png`). You can pass `ch.rhino.RhinoPlayer-master-1024.png` to re-bake hicolor from the current master (e.g. after hand edits).
2. **Fuzz %** (default `8`) for `transparent white` — high enough for off-white and anti-aliased edges; lower if the squircle is eaten, raise if a fringe of background remains.
3. **Inset** (default `0`) — for the 1024 master only, scale art to `(1 - 2×inset) × 1024` and center, leaving a **transparent** frame (optional GNOME / dock “breathing room”). `0` = **full bleed** in the 1024 square.

Pipeline (conceptually):

1. **Remove “paper”:** `-fuzz N% -transparent white` (near-uniform off-white is removed).
2. **Trim** to the visible logo bounds, then **force square** with `-extent` to `S×S` with `S = max(w, h)` (no stretching; only transparent padding on the short axis if needed).
3. **Scale** the square to **1024×1024** (uniform: large sources shrink, small sources upscale), replacing the old step that fitted into **800×800** and centered in 1024 (which left large empty margins).
4. **Write** `source/ch.rhino.RhinoPlayer-master-1024.png` and all `hicolor/…/apps/…` sizes. PNGs use reasonable compression; if **`optipng`** is on `PATH`, it is run for an extra lossless squeeze.

After **system install**, refresh the cache:

`sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor`

For this repo, `src/icons.rs` prepends the manifest `data/icons` path at runtime, and `data/install-to-user-dirs.sh` can copy the tree to `~/.local/share` for the shell (taskbar / alt+tab) icon.

[icon theme]: https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html
[GApplication id]: https://developer.gnome.org/documentation/tutorials/application-id.html
[`.desktop`]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
[ImageMagick]: https://imagemagick.org/
