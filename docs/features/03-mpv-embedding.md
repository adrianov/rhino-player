# mpv embed: render context and video surface

**Name:** mpv render integration (libmpv + OpenGL)

**Implementation status:** Done

**Use cases:** Video and audio render inside the app window on typical Linux desktops; users get standard mpv behavior without a separate player window.

**Short description:** Embed mpv using `vo=libmpv` and a render API connection to a GTK `gtk::gla::GLArea` (or equivalent) with platform display handles for X11 and Wayland.

**Long description:** Implementation uses the official libmpv C API (Rust bindings) to create a render context, flip Y for OpenGL, read the current framebuffer, and repaint on `render-update`. A secondary “null” mpv instance is optional for thumbnail preview. Hardware decode and NVIDIA quirks (graphics offload) should be considered after the basic path works with software or auto hwdec.

**Current code:** `src/mpv_embed.rs` — `libmpv2` `RenderContext` with EGL `eglGetProcAddress`, `libGL` `glGetIntegerv` (`GL_FRAMEBUFFER_BINDING`), `RenderParam::FlipY(true)` on draw, `video-timing-offset=0` so GTK paint is not blocked by mpv timing waits, update callback → `queue_render` on the main context, and `report_swap` from GTK frame-clock `after-paint` so mpv is told after GTK presents the frame. Wayland/X11 display pointers in `RenderParam` may be added if needed for specific GPUs. Audio: `ao=pulse` in the initializer (PipeWire’s Pulse compat on typical GNOME systems).

**Specification:**

- mpv is configured with `vo=libmpv`, OSC off, and internal bindings loaded from a memory buffer or file (see [Input shortcuts](13-input-shortcuts.md)).
- When the XDG config path exists, set `save-position-on-quit`, `watch-later-dir` (`~/.config/rhino/watch_later`), and `write-filename-in-watch-later-config` so resume keys match real paths. Before opening another file, the app flushes the outgoing file with `write-watch-later-config` and DB snapshot—**except** when playback reached a **natural end** (EOF or within ~3s of a known `duration`): then the app **removes** that file’s watch_later sidecar and clears SQLite `time_pos` so the next open starts at **0** (including re-opening the same file, sibling next/prev, and Escape / quit).
- A GL area fills the video region; on realize, create render context; on render, pass FBO size accounting for scale factor; request redraw on mpv’s update callback. Playback timing is applied in [26-sixty-fps-motion](26-sixty-fps-motion.md); the removed display-resample path is documented in [25-smooth-playback](25-smooth-playback.md).
- If GPU vendor is NVIDIA, allow disabling `Gtk.GraphicsOffload` equivalent if it breaks rendering.
- When idle (no file), show a start/status page; when playing, show GL area (see [Application shell](02-application-shell.md) and window state).
