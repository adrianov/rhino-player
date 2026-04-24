# Playlist: queue, prev/next, shuffle, loop

**Name:** Playlist behavior

**Implementation status:** Not started

**Use cases:** Binge a series, shuffle music, loop one tutorial, or loop a whole folder without re-opening files.

**Short description:** Queue multiple items, navigate previous/next, shuffle playlist, loop single file or whole playlist, and keep playback after EOF per mpv’s `keep-open` policy.

**Long description:** mpv’s playlist is the source of truth. The UI shows shuffle/loop toggles, enables prev/next when multiple items exist or when shuffle/loop makes navigation always valid, and at end of single file with `keep-open` may rewind to start and pause. Opening “replace” vs “append-play” is specified in open/drag/CLI features.

**Specification:**

- `playlist-pos`, `playlist-count`, and `playlist` introspection to drive button sensitivity.
- `playlist-shuffle` / `playlist-unshuffle` on shuffle toggle; `loop-file` and `loop-playlist` are mutually exclusive in UI (activating one clears the other’s infinite mode).
- Prev at first item goes to last; next at last goes to first (wrap) when list has more than one item.
- `eof-reached` / idle handling coordinates with [Session](16-session-persistence.md) and window close behavior.
