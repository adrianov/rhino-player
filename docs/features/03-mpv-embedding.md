# mpv embed: render context and video surface

**Name:** mpv render integration (libmpv + OpenGL)

**Implementation status:** In progress

**Use cases:** Video and audio render inside the app window on typical Linux desktops; users get standard mpv behavior without a separate player window.

**Short description:** Embed mpv using `vo=libmpv` and a render API connection to a GTK `gtk::gla::GLArea` (or equivalent) with platform display handles for X11 and Wayland.

**Long description:** Implementation uses the official libmpv C API (Rust bindings) to create a render context, flip Y for OpenGL, read the current framebuffer, and repaint on `render-update`. A secondary “null” mpv instance is optional for thumbnail preview. Hardware decode and NVIDIA quirks (graphics offload) should be considered after the basic path works with software or auto hwdec.

**Current code:** `src/mpv_embed.rs` — `libmpv2` `RenderContext` with EGL `eglGetProcAddress`, `libGL` `glGetIntegerv` (`GL_FRAMEBUFFER_BINDING`), `RenderParam::FlipY(true)` on draw, `report_swap`, update callback → `queue_render` on the main context. Wayland/X11 display pointers in `RenderParam` may be added if needed for specific GPUs. Audio: `ao=pulse` in the initializer (PipeWire’s Pulse compat on typical GNOME systems).

**Specification:**

- mpv is configured with `vo=libmpv`, OSC off, and internal bindings loaded from a memory buffer or file (see [Input shortcuts](13-input-shortcuts.md)).
- A GL area fills the video region; on realize, create render context; on render, pass FBO size accounting for scale factor; request redraw on mpv’s update callback.
- If GPU vendor is NVIDIA, allow disabling `Gtk.GraphicsOffload` equivalent if it breaks rendering.
- When idle (no file), show a start/status page; when playing, show GL area (see [Application shell](02-application-shell.md) and window state).
