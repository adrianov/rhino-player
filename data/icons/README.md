# Icons (Freedesktop / GNOME)

## Layout

- `hicolor/<NxN>/apps/ch.rhino.RhinoPlayer.png` — [icon theme] assets; name matches [GApplication id] `ch.rhino.RhinoPlayer` and the `Icon=` key in the [`.desktop`] file.
- `source/ch.rhino.RhinoPlayer-source-gemini-1024.png` — **unmodified** 1024×1024 design export (arbitrary background; for archives only).
- `source/ch.rhino.RhinoPlayer-master-1024.png` — **processed** 1024×1024, transparent background, on-square, safe margins — used to regenerate all hicolor sizes.

## Regenerating (ImageMagick)

Run **`rebuild-png-assets.sh`** from the project root (or this directory):

```bash
./data/icons/rebuild-png-assets.sh data/icons/source/ch.rhino.RhinoPlayer-source-gemini-1024.png 8
```

The second argument is **fuzz %** (default `8`) for `transparent white` — high enough for off-white and anti-aliased edges; lower if the squircle is eaten, raise if a fringe of background remains. Pipeline:

1. **Remove background:** `-fuzz N% -transparent white` (near-uniform “paper” is treated as white in practice).
2. **Trim** opaque bounds, then **force square** with transparent bands using `extent` to `max(w,h) × max(w,h)` (no stretching; result is always square before scaling).
3. **Fit** art to at most 800×800, **center** on 1024×1024 (GNOME “safe” margin).
4. **Write** `source/ch.rhino.RhinoPlayer-master-1024.png` and all `hicolor/…/apps/…` sizes (16 … 1024, including 22).

After **system install**, refresh the cache:

`sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor`

For this repo, `src/icons.rs` prepends the manifest `data/icons` path at runtime, and `data/install-to-user-dirs.sh` can copy the tree to `~/.local/share` for the shell (taskbar / alt+tab) icon.

[icon theme]: https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html
[GApplication id]: https://developer.gnome.org/documentation/tutorials/application-id.html
[`.desktop`]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
[ImageMagick]: https://imagemagick.org/
