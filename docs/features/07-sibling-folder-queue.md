# Sibling folder queue (folder playback)

**Name:** Sibling media expansion

**Implementation status:** Not started

**Use cases:** Open one episode or track and still use Prev/Next to move through the rest of the same folder in sorted order.

**Short description:** When a single local file is opened, discover other video/audio/image files in the same directory (sorted, excluding common subtitle extensions), and replace the queue with a generated playlist so Prev/Next walks the folder.

**Long description:** List sibling files with MIME filtering, build a temporary m3u8, `loadfile` replace, restore position to the originally opened file, then re-run shuffle if needed. The temp m3u can be removed after a short delay.

**Specification:**

- Trigger when `playlist_count == 1` and path is a local file.
- Sibling list: same directory, sorted; exclude common subtitle extensions; only `video/`, `audio/`, `image/` MIME.
- If fewer than two valid siblings after filtering, do nothing.
- After loading m3u, set `playlist-pos` to the index of the original file.
- Reuse shuffle/playlist sync logic with [Playlist](05-playlist.md).
