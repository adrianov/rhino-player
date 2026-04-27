# Open files: file picker, folder, CLI, single-instance

**Name:** Open files, folders, and command-line integration

**Implementation status:** In progress — **Open Video** dialog + **optional first `argv` path** on launch; not yet: drag-and-drop, `HANDLES_OPEN` / remote activation, open-folder-as-playlist, `--new-window`, single-instance policy.

**Use cases:** Open from the file manager, drag files in, or pass paths on the command line; use one running window or several, depending on preference.

**Short description:** File dialogs to open or add media, open folders as playlists, optional URL dialogs, and handling `GApplication`’s `open` and remote activation with a `--new-window` style flag.

**Long description:** Users open files (clear+replace or add), add subtitles/audio externally, open folders (directory as playlist in mpv), and paste/enter URLs. If the app is already running, opening files should append to the current window or open a new window per preference. Remote activation should return a message when `--new-window` is not used. Optional: probe first video with ffprobe to size the window — can be a follow-up. Recursive “find first playable file” when passed a directory from the CLI is useful for desktop entry points.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Open files and CLI integration (target behavior — partly implemented)
  Scenario: Video-only Open dialog
    Given the user chooses Open Video from the shell
    When the dialog is presented
    Then listed extensions match documented video suffixes and exclude unrelated types until separate flows ship

  Scenario: Secondary activation respects window preference
    Given another instance activates with paths while open-new-windows is off
    When files are handed to the running application
    Then loads target the active window per documented replace-or-append rules

  Scenario: Command-line startup path
    Given the user launches with supported argv paths
    When the first window paints without conflicting session rules
    Then those paths load instead of the empty-state recent grid where applicable
```

- The main **Open** dialog lists **video** only (`video/*` plus common video suffixes), not still images. Separate flows (later) for subtitle and external audio.
- `HANDLES_OPEN` or Rust equivalent: activate app, load paths with `append-play` or `replace` per action.
- `open-new-windows` preference: when off, new files go to the active window (stop current and append/replace per product rules for “open from outside”).
- Command-line: `--new-window` where applicable for secondary instances.
