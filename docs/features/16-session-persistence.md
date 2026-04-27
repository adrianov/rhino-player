# Session: restore last playlist

---
status: planned
priority: p2
layers: [fs, mpv]
related: [05, 06, 14, 21]
settings: [save-session]
---

## Use cases
- Pick up a long watch session after a reboot.
- Restore the same queue when reopening the app.

## Description
On clean exit, when `save-session` is true, the app writes the current playlist as `#EXTM3U` to a file under the XDG config directory. On the first window activation, it loads that file with `replace`. An empty queue removes the file. When CLI paths or [21-recent-videos-launch](21-recent-videos-launch.md) apply, the spec records which one wins for first paint.

## Behavior

```gherkin
@status:planned @priority:p2 @layer:fs @area:session
Feature: Session playlist restore

  Scenario: Save session on clean exit
    Given save-session is true and the playlist is non-empty
    When the application exits cleanly
    Then an m3u8 playlist file is written under the documented XDG config path

  Scenario: Restore session on first window
    Given save-session is true and a saved playlist file exists
    When the first window activates with no CLI paths and no other takeover
    Then the saved playlist is loaded with replace

  Scenario: Empty playlist removes stale file
    Given save-session is true but the queue is empty at exit
    When session save runs
    Then the last-playlist file is removed or absent

  Scenario: CLI paths take priority over restore
    Given save-session is true and CLI paths are present
    When the first window activates
    Then CLI paths load instead of the saved playlist
```

## Notes
- Path: `~/.config/rhino/last-playlist.m3u8` (mirrors XDG).
- Restore only on the first window in multi-window scenarios.
