# Open files: file picker, folder, CLI, single-instance

**Name:** Open files, folders, and command-line integration

**Implementation status:** Not started

**Use cases:** Open from the file manager, drag files in, or pass paths on the command line; use one running window or several, depending on preference.

**Short description:** File dialogs to open or add media, open folders as playlists, optional URL dialogs, and handling `GApplication`’s `open` and remote activation with a `--new-window` style flag.

**Long description:** Users open files (clear+replace or add), add subtitles/audio externally, open folders (directory as playlist in mpv), and paste/enter URLs. If the app is already running, opening files should append to the current window or open a new window per preference. Remote activation should return a message when `--new-window` is not used. Optional: probe first video with ffprobe to size the window — can be a follow-up. Recursive “find first playable file” when passed a directory from the CLI is useful for desktop entry points.

**Specification:**

- File filters: video, audio, image for generic open; separate flows for subtitle and external audio.
- `HANDLES_OPEN` or Rust equivalent: activate app, load paths with `append-play` or `replace` per action.
- `open-new-windows` preference: when off, new files go to the active window (stop current and append/replace per product rules for “open from outside”).
- Command-line: `--new-window` where applicable for secondary instances.
