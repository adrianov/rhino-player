# MPRIS2 (media keys, shell integration)

---
status: planned
priority: p2
layers: [platform, mpv]
related: [02, 04, 22]
---

## Use cases
- Media keys, notification area, and desktop widgets control playback.
- Other apps can query "what is playing".

## Description
The app exposes `org.mpris.MediaPlayer2` and `org.mpris.MediaPlayer2.Player` on the session bus and synchronises them with the active window’s mpv state. Properties update via `PropertiesChanged`; jumps emit `Seeked`. `Raise` presents the window; `Quit` quits the app.

## Behavior

```gherkin
@status:planned @priority:p2 @layer:platform @area:mpris
Feature: MPRIS2 shell integration

  Scenario: PlayPause toggles active playback
    Given Rhino exposes org.mpris.MediaPlayer2.Player on the session bus
    When a client invokes PlayPause
    Then playback toggles to match mpv pause semantics
    And PlaybackStatus reflects the new state

  Scenario: Metadata tracks current media
    Given media with identifiable metadata is playing
    When a client reads PlaybackStatus, Metadata, and Position
    Then returned values match the active window’s mpv-backed state within documented tolerance

  Scenario: Raise brings the window forward
    Given the main window is in the background
    When a client invokes Raise
    Then the application presents its primary window per GNOME expectations

  Scenario: Seeked emits on jumps
    Given playback is active
    When the user or a client causes a non-incremental position change
    Then the Seeked signal fires with the new position
```

## Notes
- Bus name and object path follow MPRIS conventions; the identity string uses the Rhino app name.
- `SetPosition` and `Seek` use microsecond contracts per spec.
- Sync via mpv property events, not per-frame polling.
