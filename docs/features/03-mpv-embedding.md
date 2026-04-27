# mpv embed: render context and video surface

**Name:** mpv render integration (libmpv + OpenGL)

**Implementation status:** Done

**Use cases:** Video and audio render inside the app window on typical Linux desktops; users get standard mpv behavior without a separate player window.

**Short description:** Embed mpv using `vo=libmpv` and a render API connection to a GTK `gtk::gla::GLArea` (or equivalent) with platform display handles for X11 and Wayland.

**Long description:** Implementation uses the official libmpv C API (Rust bindings) to create a render context, flip Y for OpenGL, read the current framebuffer, and repaint on `render-update`. A secondary “null” mpv instance is optional for thumbnail preview. Hardware decode and NVIDIA quirks (graphics offload) should be considered after the basic path works with software or auto hwdec.

**Current code:** `src/mpv_embed.rs` — `libmpv2` `RenderContext` with EGL `eglGetProcAddress`, `libGL` `glGetIntegerv` (`GL_FRAMEBUFFER_BINDING`), `RenderParam::FlipY(true)` on draw, mpv’s default `video-timing-offset` (typically `0.05`) so frames have scheduling headroom before GTK/compositor presentation, and update callback → `queue_render` on the main context. The app does not call optional `report_swap`; incorrect compositor timing feedback can make playback cadence worse than mpv’s default render scheduling. Wayland/X11 display pointers in `RenderParam` may be added if needed for specific GPUs. Audio: `ao=pulse` in the initializer (PipeWire’s Pulse compat on typical GNOME systems). **Event loop:** `MpvBundle::observe_props` + `install_event_drain` (uses `mpv_set_wakeup_callback`) hop the wakeup back onto the GTK main thread via `MainContext::invoke`; the consumer drains with `wait_event(0)` until empty (`drain_events`). `src/app/transport_events.rs` consumes those events to keep play/pause/seek/volume/mute/duration UI mirrors and EOF / sibling advance event-driven (see `.cursor/rules/events-over-polling.mdc`). The `path` property is observed too, so Prev/Next button sensitivity + tooltip refresh on every load (no transport poll). When observers are first installed (after the GLArea realize), the play button, sibling-nav buttons, and seek range are seeded from `mpv.get_property` immediately — `mpv_observe_property` only emits the initial value on a later wakeup, so this prevents stale UI when a continue-card warm preload finishes loading **before** observers exist. Seek-slider redraws driven by `time-pos` are rate-limited to ~10 Hz (`TIME_POS_MIN_GAP`) so adjacent chrome controls don’t flicker their tooltip popups when the slider repaints every video frame; while the bottom bar is hidden by autohide (and the recent grid is not visible), `time-pos` events skip both the seek-slider and time-label writes — invisible chrome should not be invalidated. Time labels are not throttled when the bar is visible (their formatted string changes ≤ 1 Hz so `set_label` is already a no-op between seconds).

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Embedded mpv video surface
  Scenario: Resume vs fresh start after natural end
    Given watch-later and SQLite resume integration are configured as specified
    When playback reaches natural end for the current file (EOF or near-end rule)
    Then resume/watch-later sidecars are cleared so the next open may start from the beginning

  Scenario: Video region visibility
    Given no media is loaded
    When the empty state or placeholder applies per shell rules
    Then the GL video surface shows the documented idle/start presentation instead of opaque playback

  Scenario: Paused seek with heavy video filter
    Given a frame-interpolation or external vf graph may alter paused frames
    When the user seeks while paused under conditions where still-frame fallback is required
    Then the UI avoids presenting an unintended black frame per documented vf handling
```

- mpv is configured with `vo=libmpv`, OSC off, and internal bindings loaded from a memory buffer or file (see [Input shortcuts](13-input-shortcuts.md)).
- When the XDG config path exists, set `save-position-on-quit`, `watch-later-dir` (`~/.config/rhino/watch_later`), and `write-filename-in-watch-later-config` so resume keys match real paths. Before opening another file, the app flushes the outgoing file with `write-watch-later-config` and DB snapshot—**except** when playback reached a **natural end** (EOF or within ~3s of a known `duration`): then the app **removes** that file’s watch_later sidecar and clears SQLite `time_pos` so the next open starts at **0** (including re-opening the same file, sibling next/prev, and Escape / quit).
- A GL area fills the video region; on realize, create render context; on render, pass FBO size accounting for scale factor; request redraw on mpv’s update callback. Playback timing is applied in [26-sixty-fps-motion](26-sixty-fps-motion.md); the removed display-resample path is documented in [25-smooth-playback](25-smooth-playback.md).
- If GPU vendor is NVIDIA, allow disabling `Gtk.GraphicsOffload` equivalent if it breaks rendering.
- When idle (no file), show a start/status page; when playing, show GL area (see [Application shell](02-application-shell.md) and window state).
