# Window: size, fullscreen, UI auto-hide, inhibit idle

**Name:** Window and presentation

**Implementation status:** In progress (fullscreen + overlay + pointer idle)

**Use cases:** Immersive fullscreen; chrome hides when not needed; the screen does not lock during a movie; optional sizing matches video resolution.

**Short description:** Default window size, optional maximize restore, fullscreen sync with mpv, header/controls autohide after idle, cursor hide in fullscreen, and idle inhibitor while playing to prevent screen lock.

**Long description:** Use a revealer or equivalent to hide chrome, motion on header/controls to show, timers to hide after a few seconds, and `GtkApplication.inhibit` for idle when not paused. Fullscreen: mpv and GTK stay in sync; header decoration in fullscreen can reduce to close only. Optional: set stream properties for the audio sink icon (platform-specific). Optional: window sized from first file resolution via `ffprobe` (see [Open and CLI](06-open-and-cli.md)).

**Specification:**

- `inhibit` when `not pause` and not idle; remove when pausing or idle.
- Autohide timeout default (e.g. 2s) and exceptions when popovers or menus are open.
- **Escape** leaves fullscreen; when not fullscreen, it can return to the recent-videos view (see [Input shortcuts](13-input-shortcuts.md)).
- **Enter** / **numpad Enter** toggles fullscreen (same behavior as double-click on the video surface).
- Fullscreen button tooltip updates with state when that control exists.
- Double primary-click on the video surface (`GLArea`) toggles fullscreen: windowed → fullscreen, fullscreen → windowed. Before entering fullscreen, the window is unmaximized if needed so the compositor does not keep a maximized layout. While fullscreen, `AdwToolbarView` top and bottom bars are hidden so the video is edge-to-edge, not a large window with visible chrome.
- In **fullscreen**, any pointer motion (capture phase on the main window) reveals the top and bottom bars; after **3 seconds** without motion they hide again. While fullscreen, `AdwToolbarView` has **extend content to top and bottom edge** so the `GLArea` is allocated the full area and the **header and bottom toolbars are drawn on top of the video** (overlay), not as extra vertical strips that shrink the video.
- On the **video `GLArea`** (windowed and fullscreen), the pointer is hidden after **3 seconds** without moving over that area (`set_cursor_from_name("none")`, plus a CSS class for styling if needed); moving the pointer over the `GLArea` shows it again. Leaving the `GLArea` cancels the hide timer and shows the default cursor. After the fullscreen chrome auto-hides, a short **layout squelch** and duplicate **(x, y) filtering** avoid spurious motion/enter from reflow, which would otherwise re-open the toolbars and keep the pointer visible.
