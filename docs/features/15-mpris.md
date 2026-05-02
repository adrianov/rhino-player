# MPRIS2 shell integration

---
status: done
priority: p2
layers: [playback, os-integration]
related: [02, 04, 13, 07]
scope: platform-specific
---

## Use cases

- Pause, play, next, and previous from the desktop environment reach the focused player reliably.
- The shell can show basic “now playing” state for the active item.

## Description

Linux builds register on the desktop session integration bus used by shells and peripherals for media controls, with properties kept in rough sync with playback. Other ports skip this path.

## Behavior

```gherkin
@status:done @priority:p2 @layer:os-integration @area:mpris
Feature: MPRIS2 shell integration

  Scenario: PlayPause toggles active playback
    Given Linux transport has loaded playable media with a finite length
    When a session client invokes the standard play-pause toggle for this application’s media session
    Then pause state toggles for the loaded item
    And the reported playing vs paused classification matches playback state shortly after

  Scenario: Dedicated play and pause
    Given Linux transport has loaded playable media with a finite length
    When a session client invokes dedicated play control
    Then playback resumes if it was paused
    When a session client invokes dedicated pause control or stop-style halt
    Then playback is held at the current position without unloading the item

  Scenario: Folder queue previous and next
    Given sibling folder advances are available before and after the current local item
    When a session client invokes previous-track or next-track control
    Then the previous or next sibling item is loaded respectively

  Scenario: Raise brings the primary window forward
    Given the primary window exists
    When a session client invokes raise for this application’s media session
    Then the primary window is presented ahead of sibling windows where the toolkit allows

  Scenario: Quit ends the shell session cleanly
    When a session client requests application quit for this media session
    Then the application shuts down via its normal lifecycle
```

## Notes

- Implemented only for `cfg(target_os = "linux")` in `Cargo.toml` `[target.'cfg(target_os = "linux")'.dependencies]` (`mpris-server`, `async-channel`, `futures`). Module: `src/mpris/` (`linux.rs`: `Player`, `glib::spawn_future_local`, channel + `async` apply loop synchronized with mpv seek helpers).
- Bus name suffix `RhinoPlayer_<pid>`; object path follows MPRIS defaults from the crate. Controls delegate to existing play/pause, sibling-folder load, `main_player_seek_keyframes`; transport (`dispatch_event` + `transport_tick`) publishes `enqueue_snapshot`.
- Raise / Quit: `present` / `Application::quit` on GTK main idle.
- Relative seek, absolute position set (`SetPosition`), and emitted `Seeked` after programmatic jumps from session clients share the GUI seek pathway (drops smooth-motion `vf` like the seek bar).
- `desktop_entry` property uses the Rhino application id (`ch.rhino.RhinoPlayer`).
