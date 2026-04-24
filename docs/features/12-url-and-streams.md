# URL and network streams (yt-dlp / protocols)

**Name:** URL playback

**Implementation status:** Not started

**Use cases:** Play web streams and network URLs the same way as local files, with minimal friction.

**Short description:** Open network URLs and use yt-dlp (or the hook equivalent) for supported sites; validate schemes against `protocol-list` where applicable.

**Long description:** A small Adwaita dialog with URL entry: accept `mpv`-supported schemes, or bare hostnames (prepend `https://`), or existing filesystem paths. “Open” vs “Add” maps to `replace` vs `append-play`. mpv is configured with `ytdl` enabled and scripts as needed for stream extraction.

**Specification:**

- Parse URL, validate, call `loadfile` with the correct mode.
- If the playlist dialog is open, refresh it after add (same window flow as other queue updates).
