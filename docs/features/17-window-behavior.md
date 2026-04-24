# Window: size, fullscreen, UI auto-hide, inhibit idle

**Name:** Window and presentation

**Implementation status:** Not started

**Use cases:** Immersive fullscreen; chrome hides when not needed; the screen does not lock during a movie; optional sizing matches video resolution.

**Short description:** Default window size, optional maximize restore, fullscreen sync with mpv, header/controls autohide after idle, cursor hide in fullscreen, and idle inhibitor while playing to prevent screen lock.

**Long description:** Use a revealer or equivalent to hide chrome, motion on header/controls to show, timers to hide after a few seconds, and `GtkApplication.inhibit` for idle when not paused. Fullscreen: mpv and GTK stay in sync; header decoration in fullscreen can reduce to close only. Optional: set stream properties for the audio sink icon (platform-specific). Optional: window sized from first file resolution via `ffprobe` (see [Open and CLI](06-open-and-cli.md)).

**Specification:**

- `inhibit` when `not pause` and not idle; remove when pausing or idle.
- Autohide timeout default (e.g. 2s) and exceptions when popovers or menus are open.
- `Escape` leaves fullscreen; fullscreen button tooltip updates with state.
