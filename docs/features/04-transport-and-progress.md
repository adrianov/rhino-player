# Transport: play, pause, seek, progress UI

**Name:** Transport controls and progress bar

**Implementation status:** Done (seek + time, bottom bar prev / play / next; **speed** 1.0× / 1.5× / 2.0× in **header** — [28](28-playback-speed.md); prev/next per sibling order — [07](07-sibling-folder-queue.md); volume — [22](22-audio-volume-mute.md); shuffle/loop: [05](05-playlist.md))

**Use cases:** Control playback without leaving the app; see position and total length; adjust volume; enter fullscreen for focused viewing.

**Short description:** Play/pause, time labels, seek bar bound to `time-pos` / `duration`, volume control and mute, and optional “elapsed vs remaining time” display.

**Long description:** The header/toolbar or overlay hosts transport buttons consistent with Adwaita: previous, play/pause, next (when playlist allows), volume menu with scale (including over-100% if `volume-max` > 100), and fullscreen toggle. The seek bar is user-adjustable; while dragging, avoid feedback loops. Large durations format in a human-friendly way (including days/hours when needed). Scroll on the bar may adjust position (smoothed/scroll event handling). Optional: click-hold on primary mouse button to temporary speedup (2×) with OSD text.

**Specification:**

- Properties observed: `time-pos`, `duration`, `pause`, `mute`, `volume`, `volume-max`, `speed`, `fullscreen` (or window fullscreen state), `media-title` for window title. **Speed** 1.0× / 1.5× / 2.0× is in the **header** only (left of subtitle/volume/main-menu popovers; see [28](28-playback-speed.md)). In the **bottom** bar (LTR order): **Previous** and **Next** to skip by sibling-folder rules when applicable (see [07](07-sibling-folder-queue.md)), then play/pause, then elapsed and seek. Play/pause is `sensitive` only when `duration` > 0, toggles `pause` (same as Space; see [Input shortcuts](13-input-shortcuts.md)). **Previous/Next** are `sensitive` when a skip target exists in that order (often disabled at the first or last file in the chain). **Speed** in the header is `sensitive` when the seek bar is (duration > 0).
- Seek bar: upper bound = duration; disabled when duration unknown/zero. User seeks use mpv `seek <seconds> absolute+keyframes` (fallback: setting `time-pos`) so audio/video stay aligned, including with filtered video. When playback is paused and the active `vf` contains VapourSynth, the app temporarily clears that `vf` before the seek so mpv can render a normal paused still frame instead of a black filtered surface; Smooth 60 is reapplied on the next Play/Space unpause if the preference is still enabled. Optional **hover preview**: popover with a **thumbnail** of the frame at the hover time; see [18-thumbnail-preview](18-thumbnail-preview.md) (toggled in **Preferences**).
- User setting toggles between elapsed and negative remaining time (`show-remaining`).
- Match at least 5s/10s style keyboard seeks via [Input shortcuts](13-input-shortcuts.md).
