# Icons (Freedesktop / GNOME)

## Layout

- `hicolor/<NxN>/apps/ch.rhino.RhinoPlayer.png` — [icon theme] assets; name matches [GApplication id] `ch.rhino.RhinoPlayer` and the `Icon=` key in the [`.desktop`] file. Standard sizes: **16, 22, 24, 32, 48, 64, 128, 256, 512, 1024**.
- `source/ch.rhino.RhinoPlayer-source-800.png` — **unmodified** design export (white margin around the squircle). Used as the default input to `rebuild-png-assets.sh`.
- `source/ch.rhino.RhinoPlayer-master-1024.png` — **processed** 1024×1024: **transparent** background (white only where it connects to the **image border**; interior whites stay), art **trimmed** to content, **squared** to the minimal bounding square (by extent, no stretch), then **scaled to 1024×1024**, then all hicolor sizes are generated from this.

## Regenerating (ImageMagick)

Run **`rebuild-png-assets.sh`** from the project root (or this directory). Default first argument is `source/ch.rhino.RhinoPlayer-source-800.png`; output is the master and every `hicolor/…/ch.rhino.RhinoPlayer.png`.

```bash
./data/icons/rebuild-png-assets.sh
# or a different design export (same `Icon=` name):
./data/icons/rebuild-png-assets.sh /path/to/rhino-logo.png 4
# third arg: inset (0.00–0.12) to leave a small transparent margin inside 1024
./data/icons/rebuild-png-assets.sh data/icons/source/ch.rhino.RhinoPlayer-source-800.png 4 0.04
```

Arguments:

1. **Source PNG** (default: `data/icons/source/ch.rhino.RhinoPlayer-source-800.png`). You can pass `ch.rhino.RhinoPlayer-master-1024.png` to re-bake hicolor from the current master (e.g. after hand edits). Any size works: it is **trimmed**, **squared**, then **scaled to 1024** before hicolor.
2. **Fuzz %** (default `4`) for the **flood fill** that removes [near-] white **connected to the four image corners** (same as edge-touching “paper” background). Lower if the icon edge fringes, raise (e.g. 8) if a bit of off-white background remains. **This is not a global** `-transparent white` (which would also remove many white design elements on the art).
3. **Inset** (default `0`) — for the 1024px master only, scale art to `(1 - 2×inset) × 1024` and center, leaving a **transparent** frame (optional GNOME / dock “breathing room”).

Pipeline (conceptually):

1. **Remove outer margin:** four **flood-fills** from the corners, `-fuzz N% -fill none -floodfill ± - white`, so only **white (and similar) that connects to a corner** goes transparent. White **inside** the icon (e.g. a play chevron) usually stays, because the rhino body blocks path from the corners.
2. **Trim** to the visible logo bounds, then **force square** with `-extent` to `S×S` with `S = max(w, h)` (no stretching; only transparent padding on the short axis if needed).
3. **Scale** the square to **1024×1024** (uniform), replacing a prior step that used global `-transparent white` (too aggressive for this logo).
4. **Write** `source/ch.rhino.RhinoPlayer-master-1024.png` and all `hicolor/…/apps/…` sizes. If **`optipng`** is on `PATH`, it is run for an extra lossless squeeze.

After **system install**, refresh the cache:

`sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor`

For this repo, `src/icons.rs` prepends the manifest `data/icons` path at runtime, and `data/install-to-user-dirs.sh` can copy the tree to `~/.local/share` for the shell (taskbar / alt+tab) icon.

[icon theme]: https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html
[GApplication id]: https://developer.gnome.org/documentation/tutorials/application-id.html
[`.desktop`]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
[ImageMagick]: https://imagemagick.org/
