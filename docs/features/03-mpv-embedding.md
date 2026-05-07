# mpv embed: render context and video surface

---
status: done
priority: p0
layers: [mpv, ui]
related: [02, 14, 17, 18, 26]
mpv_props: [time-pos, duration, pause, eof-reached, path, video-timing-offset, save-position-on-quit, watch-later-dir, write-filename-in-watch-later-config]
---

## Use cases
- Video and audio render inside the app window on typical Linux desktops.
- Users get standard mpv behaviour without a second player window.
- Resume picks up watch positions across launches for the same paths.

## Description
mpv is embedded with `vo=libmpv` and an OpenGL render context bound to a `gtk::GLArea`. Y is flipped on draw; the framebuffer binding is read on each render. Repaints are triggered by mpv’s `render-update` callback. mpv events arrive through a wakeup callback that hops to the GTK main thread and drains `wait_event(0)` until empty; the app reacts to property changes (`time-pos`, `duration`, `pause`, `path`, …) instead of polling.

The XDG config tree owns its own `watch_later` directory so resume keys match real paths. Natural end-of-playback (EOF or within ~3s of a known `duration`) clears the watch-later sidecar and SQLite resume so the next open starts at zero.

## Behavior

```gherkin
@status:done @priority:p0 @layer:mpv @area:embed
Feature: Embedded mpv video surface

  Scenario: Resume after partial playback
    Given save-position-on-quit is on and watch-later-dir points at the app config directory
    When the user quits the app while a local file is paused mid-stream
    Then a sidecar file appears in watch-later-dir
    And reopening the same path resumes from the saved time

  Scenario: Natural EOF clears resume
    Given a local file is playing
    When playback reaches natural end (EOF or within ~3s of duration)
    Then the watch-later sidecar for that path is removed
    And SQLite time_pos for that path is cleared

  Scenario: Idle state shows no opaque playback frame
    Given no media is loaded
    When the empty state applies per shell rules
    Then the GL surface shows the documented idle presentation, not an opaque last frame

  Scenario: Paused seek with VapourSynth vf
    Given vf contains a vapoursynth filter chain and the player is paused
    When the user seeks
    Then the app temporarily clears the vf so a normal still frame renders
    And Smooth 60 is reapplied on the next unpause if the preference remains enabled

  Scenario: Property events drive UI without a transport poll
    Given mpv events arrive via the wakeup callback
    When time-pos, pause, duration, or path change
    Then the relevant UI control updates in response to that event only
    And no polling timer rewrites the same value
```

## Notes
- Render context: `libmpv2` `RenderContext`, OpenGL init via `src/mpv_embed/gl_platform.rs`: Linux uses EGL `eglGetProcAddress` + `libGL` for `GL_FRAMEBUFFER_BINDING`; macOS resolves GL after GTK’s `GLArea` realizes using `dlsym(RTLD_DEFAULT, …)` (same `RenderParam::FlipY` path as Linux).
- mpv defaults are kept (`video-timing-offset` ≈ 0.05). **`mpv_render_context_report_swap`** is gated (**`SeqCst`**) with **`video-sync=display-resample`**: **Linux** and **macOS** use **`display-resample`** + swaps for plain **`vo=libmpv`** (**`restore_non_smooth_present_opts`**) and for Smooth **`vf`**; the gate is **off** only when **`restore`** falls back to **`audio`**. **`vf clr`** never disables swap reporting before **`video-sync`** leaves **`display-resample`**.
- **macOS native layer:** **`sync_layer_frame_now`** mirrors the **`GLArea`** allocation onto **`RhinoMpvGlLayer`** (CATransaction with implicit animations disabled). **`connect_notify_local`** on **`width`** / **`height`** plus GTK ticks (also keyed on rounded origin in window space) keep the layer aligned during live resize and chrome reflow; after each geometry commit **`DriverStateHandle::mark_pending`** nudges **`CVDisplayLink`** so **`mpv_render_context_render`** runs at the new viewport instead of stretching an old frame until the next decoded sample. **`vf clr`** from **`apply_mpv_video`** brackets **`DriverStateHandle::begin_vf_teardown`** / **`end_vf_teardown`** so **`display_now`** / CALayer **`mpv`** draws do not overlap vapoursynth teardown (Smooth **off** mid-play).
- Audio output: `ao=pulse` on Linux (PipeWire’s Pulse compat works on typical GNOME systems); `ao=coreaudio` on macOS.
- Wakeup callback installed via `mpv_set_wakeup_callback`; consumer calls `wait_event(0)` until empty.
- Seek-slider redraws driven by `time-pos` are rate-limited to ~10 Hz; while the bottom bar is hidden by autohide and the recent grid is not visible, `time-pos` events skip seek-slider and time-label writes (invisible chrome must not be invalidated).
- Observer setup seeds Play / sibling-nav / seek range from `mpv.get_property` immediately on first install so warm-preload finishes do not leave stale UI.
