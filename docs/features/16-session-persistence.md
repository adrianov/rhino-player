# Session: restore last playlist

**Name:** Last session and playlist file

**Implementation status:** Not started

**Use cases:** Pick up a long watch session after reboot; restore the same queue when reopening the app.

**Short description:** On exit, if enabled, write the current playlist to a file under the config directory; on first window activation, load that m3u to restore the session.

**Long description:** On window close or app stop, if `save-session` is true, write `#EXTM3U` and one path per line. If playlist is empty, remove the file. On startup with `save-session` and a single window, `loadfile` the m3u with `replace`. When saving, optionally enable `save-position-on-quit` to match the “restore my place” story. Restore only for the first window in multi-window scenarios.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Session playlist restore (target behavior — not started)
  Scenario: Save session on exit when enabled
    Given save-session is true and a playlist exists
    When the application exits cleanly
    Then an m3u playlist file is written under the documented XDG config path

  Scenario: Restore on startup when alone
    Given save-session is true and a saved playlist file exists
    When the first window activates without conflicting CLI or grid rules
    Then that playlist loads with replace per coordination with open-on-start features

  Scenario: Empty playlist removes stale file
    Given save-session is true but the queue is empty
    When session save runs
    Then the last-playlist file is removed or left absent as specified
```

- Path: e.g. `~/.config/rhino/last-playlist.m3u8` (TBD, mirror XDG).
- `save_last_playlist_file` and `restore_last_playlist` behaviors are defined and testable.
