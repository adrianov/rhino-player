# Sibling folder queue (folder playback)

**Name:** Sibling media expansion

**Implementation status:** In progress (EOF auto-advance; prev/next m3u not started)

**Use cases:** After one file finishes, continue with the next file in the same directory; when that folder is exhausted, continue in the next **sibling subfolder** under the same parent (e.g. next season folder) without a manual “Open” each time.

**Short description:** On natural EOF, load the next local file in **sorted** order in the current directory. If the current file was the last video there, go **up** one level, find the next **sibling directory** (sorted by name) under that parent, and start the **first** video in that directory; skip directories that have no matching videos. If the current directory was the last among siblings, **stop** (no wrap). Scope is: all videos in immediate subfolders of a common parent, in directory-name order, then by filename inside each folder.

**Long description:** Implementation uses a fixed list of video filename extensions, non-recursive per-folder listing, and the **`lexical_sort`** crate’s **`natural_lexical_cmp`** for ordering file and directory names: Unicode “folds” to ASCII, case-insensitive, with **natural** digit runs (e.g. `ep2` before `ep10`). This is a practical file-manager-style order, not [ICU](https://github.com/unicode-org/icu4x) locale collation. The trigger is mpv’s **`eof-reached`** property on the **~200ms** transport tick (with `keep-open`, **EndFile** from `wait_event` is not used—embedding did not surface it reliably). `keep-open` and watch-later work as before: `try_load` writes resume snapshot before each switch.

**Specification:**

- **Trigger:** `eof-reached` is true, once per idle end (guarded by a `Cell<bool>` that resets when `eof-reached` is false, e.g. new file or seek). The last successfully loaded canonical path from [try_load] is used if mpv’s `path` is empty.
- **Local files only** — if neither mpv’s path nor the cached last path resolves, do nothing.
- **Same directory:** list video files in the file’s **parent** (canonical paths, same extension set as the implementation), sort, advance to the next after the current file.
- **Last in directory:** list **subdirectories** of the parent of that folder, sort; take the next directory after the current one; the next play is the first (sorted) video in that directory; if that directory is empty, continue to the next sibling directory; repeat.
- **No next sibling (walk up until root):** when there is no “next” directory at any level, **stop** (no loop to the first folder).
- **Not in this slice:** a generated m3u/playlist, Prev/Next buttons, shuffle, MIME probing — may align with [Playlist](05-playlist.md) later.

**Current code:** `sibling_advance::next_after_eof` (`src/sibling_advance.rs`); `maybe_advance_sibling_on_eof` and the 200ms transport tick in `app.rs`.
