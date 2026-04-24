# Session: restore last playlist

**Name:** Last session and playlist file

**Implementation status:** Not started

**Use cases:** Pick up a long watch session after reboot; restore the same queue when reopening the app.

**Short description:** On exit, if enabled, write the current playlist to a file under the config directory; on first window activation, load that m3u to restore the session.

**Long description:** On window close or app stop, if `save-session` is true, write `#EXTM3U` and one path per line. If playlist is empty, remove the file. On startup with `save-session` and a single window, `loadfile` the m3u with `replace`. When saving, optionally enable `save-position-on-quit` to match the “restore my place” story. Restore only for the first window in multi-window scenarios.

**Specification:**

- Path: e.g. `~/.config/rhino/last-playlist.m3u8` (TBD, mirror XDG).
- `save_last_playlist_file` and `restore_last_playlist` behaviors are defined and testable.
