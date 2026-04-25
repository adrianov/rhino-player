# Sibling folder queue (folder playback)

**Name:** Sibling media expansion

**Implementation status:** Done (EOF auto-advance; **Prev/Next** on the bottom bar use the same folder + sibling ordering as EOF; full m3u playlist UI: [05](05-playlist.md))

**Use cases:** After one file finishes, continue with the next file in the same directory; when that folder is exhausted, continue in the next **sibling subfolder** under the same parent (e.g. next season folder) without a manual “Open” each time.

**Short description:** On natural EOF, load the next local file in **sorted** order in the current directory. If the current file was the last video there, go **up** one level, find the next **sibling directory** (sorted by name) under that parent, and start the **first** video in that directory; skip directories that have no matching videos. If the current directory was the last among siblings, **stop** (no wrap). Scope is: all videos in immediate subfolders of a common parent, in directory-name order, then by filename inside each folder.

**Long description:** Sibling “videos” in a folder are files whose **extension** matches the shared list in [`src/video_ext.rs`](../src/video_ext.rs) (same as **Open video…**; e.g. `mkv`, `mp4`, `mxf`, `vob`, `y4m` …). The list is **not** a probe of file contents. Listing is **non-recursive** per directory. Ordering uses the **`lexical_sort`** crate’s **`natural_lexical_cmp`**: case-insensitive Unicode to ASCII, with **natural** digit runs (e.g. `ep2` before `ep10`) — a practical file-manager-style order, not [ICU](https://github.com/unicode-org/icu4x) locale collation. The primary trigger is mpv’s **`eof-reached`** on the **~200ms** transport tick. With `keep-open` and the libmpv GL path, `eof-reached` can **stay false** while `time-pos` sits just short of `duration` (e.g. near the end for ~1s); the app also treats a **stuck** tail: unpaused, within **~1.75s** of the end, and the same `time-pos` for **3** consecutive ticks (~0.6s), then advances the same as EOF. `keep-open` and watch-later work as before: `try_load` writes resume snapshot before each switch.

**Specification:**

- **Trigger:** `eof-reached` **or** the tail-stall case above, once per logical end (guarded by `SiblingEofState` that resets when not at an end, e.g. new file, seek, or Escape). The last successfully loaded canonical path from [try_load] is used if mpv’s `path` is empty.
- **Local files only** — if neither mpv’s path nor the cached last path resolves, do nothing.
- **Same directory:** list video files in the file’s **parent** (canonical paths, same extension set as the implementation), sort, advance to the next after the current file.
- **Last in directory:** list **subdirectories** of the parent of that folder, sort; take the next directory after the current one; the next play is the first (sorted) video in that directory; if that directory is empty, continue to the next sibling directory; repeat.
- **No next sibling (walk up until root):** when there is no “next” directory at any level, **stop** (no loop to the first folder).
- **Bottom bar** **Previous** / **Next** (when a **local** file is open and has duration) use the same folder + sibling order as automatic EOF: `sibling_advance::prev_before_current` / `next_after_eof`, then `try_load` in `app.rs`; sibling EOF one-shot state is reset on manual skip. **Not in this slice** yet: a generated m3u/playlist, shuffle, MIME probing — may align with [Playlist](05-playlist.md) later.

**Current code:** `sibling_advance::next_after_eof` (`src/sibling_advance.rs`); `maybe_advance_sibling_on_eof` and the 200ms transport tick in `app.rs`.
