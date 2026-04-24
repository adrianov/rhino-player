# Transport: play, pause, seek, progress UI

**Name:** Transport controls and progress bar

**Implementation status:** Not started

**Use cases:** Control playback without leaving the app; see position and total length; adjust volume; enter fullscreen for focused viewing.

**Short description:** Play/pause, time labels, seek bar bound to `time-pos` / `duration`, volume control and mute, and optional “elapsed vs remaining time” display.

**Long description:** The header/toolbar or overlay hosts transport buttons consistent with Adwaita: previous, play/pause, next (when playlist allows), volume menu with scale (including over-100% if `volume-max` > 100), and fullscreen toggle. The seek bar is user-adjustable; while dragging, avoid feedback loops. Large durations format in a human-friendly way (including days/hours when needed). Scroll on the bar may adjust position (smoothed/scroll event handling). Optional: click-hold on primary mouse button to temporary speedup (2×) with OSD text.

**Specification:**

- Properties observed: `time-pos`, `duration`, `pause`, `mute`, `volume`, `volume-max`, `fullscreen` (or window fullscreen state), `media-title` for window title.
- Seek bar: upper bound = duration; disabled when duration unknown/zero.
- User setting toggles between elapsed and negative remaining time (`show-remaining`).
- Match at least 5s/10s style keyboard seeks via [Input shortcuts](13-input-shortcuts.md).
