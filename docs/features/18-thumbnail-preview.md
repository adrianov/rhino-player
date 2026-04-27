# Thumbnails: seek bar preview

**Name:** Thumbnail preview on the seek (navigation) bar

**Implementation status:** Done (SQLite `seek_bar_preview`; `Gtk.Popover` + framed `Gtk.GLArea` thumbnail + small centered time label; **Progress bar preview** in main menu **Preferences**; [see Cine](https://github.com/diegopvlk/Cine) for baseline UX: popover on the scale, debounced seek + frame)

**Use cases:** Scrub the timeline visually before seeking—especially for long local files.

**Short description:** On hover over the bottom seek `Gtk.Scale`, a popover above the bar shows a **framed live video thumbnail** with a small centered time label for **local** files with the option on: a **second** in-process [libmpv] with **`vo=libmpv`** and the same OpenGL [`RenderContext`] + [`gtk::GLArea`] path as the main window ([`mpv_embed::MpvBundle`]), implemented as [`mpv_embed::MpvPreviewGl`]. The mini `GLArea` is realized in the popover; `loadfile` when the path changes, no preview `vf` / filter chain, and debounced **keyframe seek** while moving. No exact follow-up seek, `screenshot-raw`, extra thread, or `MemoryTexture`. The continue grid still uses `vo=image` + DB JPEG via `media_probe` / `jpeg_texture` for on-disk cache.

**Long description:** [Cine](https://github.com/diegopvlk/Cine) uses a second [mpv] with `vo=null` and `screenshot-raw`; Rhino instead **embeds** a second player so the preview is the real GL video path. [`MpvPreviewGl`] is auxiliary only: `ao=null`, `pause=yes`, `aid=no`, `sid=no`, no external file autoload, no scripts/config, no watch-later, no resume, no preview `vf`, small demuxer cache/readahead, fast decoder flags, non-key frame skipping, and it does **not** copy main-player settings such as selected audio/subtitle tracks or `hwdec`. After `loadfile`, a bounded low-priority `glib` timeout waits until `vo-configured`, then `seek absolute+keyframes` to the hover time and `queue_render`. Each pointer move cancels the previous debounce/pump and invalidates stale callbacks, so only the latest hover position may seek the preview player. The preview intentionally skips exact seeking and main-thread event-drain loops during pointer motion so playback keeps priority. VapourSynth and `data/vs` apply only to the main player. Debounce **120** ms. Streams and non-file `path` have no thumbnail. **Progress bar preview** off hides the `GLArea` only.

**Specification:**

- **Settings:** [SQLite] key `seek_bar_preview` = `0` / `1`; default **on**; toggled from **main menu → Preferences → Progress bar preview** ([gio] stateful `seek-bar-preview`).
- **Local file:** `path` from [mpv] `path` (or the app’s `last_path` cache when mpv is empty) must resolve to a **regular** file; `http://` and similar have **no** thumbnail.
- **Placement:** Hovered time = `(x / width) * duration` in widget coordinates and drives the preview seek; the popover is **not** arrow tip (`set_has_arrow(false)`) and points to a small rect just above the pointer (`set_pointing_to`), position **Top** with a small upward offset so the framed thumbnail sits above the seek bar without overlapping it.
- **Size:** Thumbnail is intentionally small for responsiveness; long edge is responsive to the seek/window width and clamped around **180–320px**; aspect ratio follows the current video display dimensions.
- **Debounce** **120** ms. The debounce `glib::SourceId` must be taken when the timeout **runs** (GLib already removes the source on `Break`); otherwise a later cancel would call `remove` on a stale id and abort.
- **Video:** A **single** second [mpv] instance ([`MpvPreviewGl`]) in the popover; **O(1)** instances. No temp files at hover. The instance is video-only and isolated from playback settings: no audio track, no subtitle track, no external audio/subtitle autoload, no resume/watch-later. After a **new** `loadfile`, the UI waits until **`vo-configured`** (via a bounded timeout loop on the main thread) before the first `seek` for that load.
- **Not** in scope: chapter titles in the popover, URL thumbnails, a preferences **window** (menu row only, like other Rhino prefs to date).

**See also:** [Cine `window.py` / `io.github.diegopvlk.Cine`](https://github.com/diegopvlk/Cine) (different capture path, same UX target), [03-mpv-embedding](03-mpv-embedding.md), [04-transport-and-progress](04-transport-and-progress.md), [21-recent-videos-launch](21-recent-videos-launch.md) (grid `vo=image` thumbs).

[mpv]: https://mpv.io/
[libmpv2]: https://crates.io/crates/libmpv2
[SQLite]: https://www.sqlite.org/
[gio]: https://docs.gtk.org/gio/
[glib]: https://docs.gtk.org/glib/
