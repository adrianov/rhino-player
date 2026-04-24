# MPRIS2 (media keys, shell integration)

**Name:** MPRIS2 D-Bus interface

**Implementation status:** Not started

**Use cases:** Media keys, notification area, and desktop widgets control playback; other apps can query “what is playing.”

**Short description:** Expose `org.mpris.MediaPlayer2` and `org.mpris.MediaPlayer2.Player` on the session bus: play, pause, next, previous, seek, volume, loop, shuffle, and metadata, aligned with the active window’s mpv state.

**Long description:** Register a well-known D-Bus name and sync on a timer or event basis to avoid stutter from per-frame property reads. Emit `PropertiesChanged` for playback, metadata, volume, loop, shuffle, and `Seeked` when position jumps. `Raise` presents the window; `Quit` quits the app. Shuffle maps to a toggle in the main window. Next/prev capability feeds `CanGoNext` / `CanGoPrevious`.

**Specification:**

- Bus name and object path follow MPRIS conventions; identity string uses the Rhino app name.
- Methods and properties that GNOME shell and media key clients expect: at minimum `PlayPause`, `Next`, `Previous`, `Stop`, `Volume`, `LoopStatus`, `Metadata`, `Position`, `PlaybackStatus`.
- `SetPosition` and `Seek` use microsecond contracts per spec.
