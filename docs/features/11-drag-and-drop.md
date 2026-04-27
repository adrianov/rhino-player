# Drag and drop

**Name:** DnD onto video surface

**Implementation status:** Not started

**Use cases:** Add files from the file manager or browser quickly; add subtitles with minimal clicks.

**Short description:** Drop files, folders, and text/URLs onto the main video area; first item replaces, following append; subtitles route to `sub-add` when appropriate.

**Long description:** Visual drop affordance; detect subtitle extensions when something is already playing. Support `GFileList` and string URIs. After drop, re-run shuffle/playlist sync if shuffle is enabled and idle state requires it. Errors show as non-intrusive toasts or inline state.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Drag and drop onto the video surface (target behavior — not started)
  Scenario: Drop opens media
    Given the main window accepts drops
    When the user drops playable media paths onto the video area
    Then loadfile runs with replace or append-play per documented ordering rules

  Scenario: Subtitle file while playing
    Given a video is playing
    When the user drops a file that matches subtitle heuristics
    Then subtitles are added with sub-add and selection as specified
    Otherwise a new media load follows the documented replace/append rule
```

- Drop targets accept file list and plain URL string where GTK allows.
- Directory drops load as playlist (mpv `loadfile` with directory).
- If playing and file looks like a subtitle, `sub-add` with select; otherwise media `loadfile` with correct mode.
- `is_local_path` heuristics for URL vs path.
