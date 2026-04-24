# Thumbnails: seek bar preview

**Name:** Thumbnail preview on seek bar

**Implementation status:** Not started

**Use cases:** Scrub the timeline visually before jumping—especially for long files and local content.

**Short description:** On hover over the progress bar, show a small seeking preview image using a second headless mpv that seeks and runs `screenshot-raw` into a `Gdk` texture, rate-limited.

**Long description:** The preview path uses `vo=null`, `ao=null`, a scale `vf` to small BGRA dimensions, and a property observer on `time-pos` to issue `screenshot-raw` async. Only for local files when the preference is on. Debounce hover updates to limit CPU. When off or non-local, hide the picture.

**Specification:**

- Preference: `thumbnail-preview` (boolean).
- Disable for remote/URL or non-local paths.
- Throttle updates (e.g. ~70ms plus longer debounce) to keep UI responsive; exact values are implementation-tuned.
- Clean up the preview instance when closing or when disabled.
