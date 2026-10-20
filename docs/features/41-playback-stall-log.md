# Playback stall log

---
status: done
priority: p2
layers: [playback, os-integration]
related: [04, 26, 35]
---

## Use cases
- Review, after a session, every time playback stopped by itself (Smooth Video A/V resync, cache buffering, engine desync) without needing a terminal open during playback.
- See why each automatic pause happened and how long it lasted, so unnecessary pauses can be found and removed.

## Description
Whenever playback stops without the viewer asking, the app writes a line to the platform's system log and to its diagnostic output. Three kinds of stall are recorded: the app pausing playback to rebuild, remove, or reload the Smooth Video filter or to switch audio tracks; the playback engine waiting for its cache; and the engine's own audio/video desync warning.

Each pause line states the reason and gives a short snapshot of the playback state (playhead, speed, source and filter frame rates, A/V offset, busy-system hold, file name), followed by the transport events of the last few seconds that likely caused it. A second line records how long the pause lasted. If the app never resumes a pause, the next automatic pause reports it.

## Behavior

```gherkin
@status:done @priority:p2 @layer:playback @area:diagnostics
Feature: Playback stall log

  Scenario: App-driven pause is logged with reason and context
    Given a video is playing with Smooth Video on
    When the app pauses playback to rebuild, remove, reload, or resync the smoothing filter
    Then a line stating the pause reason is written to the system log
    And the line includes the playhead, frame rates, A/V offset, and file name
    And the line lists the transport events of the preceding few seconds

  Scenario: Resume records how long the pause lasted
    Given the app paused playback for a filter or audio-track resync
    When the app resumes playback
    Then a line with the same reason and the pause duration is written to the system log

  Scenario: Unresumed pause is reported
    Given the app has not resumed an earlier automatic pause
    When the app starts another automatic pause
    Then a line reports that the earlier pause was never resumed

  Scenario: Engine buffering is logged
    Given a video is playing
    When the playback engine pauses to wait for its cache
    Then a buffering line is written to the system log
    And once playback continues, a second line records how long buffering lasted

  Scenario: Engine A/V desync warning is logged
    Given a video is playing
    When the playback engine reports that audio and video have drifted out of sync
    Then the warning is written to the system log together with the preceding transport events

  Scenario: Viewer pause is not logged as a stall
    Given a video is playing
    When the viewer pauses playback
    Then no stall line is written
```

## Notes
- Owner: **`src/stall_log.rs`** — `Hold::begin` / `Hold::end`, `trigger`, `buffering`, `mpv_warning`. Each line goes to stderr as **`[rhino] stall: …`** and to **`syslog(3)`** (`LOG_USER`, `LOG_NOTICE`, ident **`rhino-player`**, message prefix **`stall:`**).
- Reading the log — Linux (journald): `journalctl -t rhino-player --since today`. macOS (unified log): `log show --last 2h --predicate 'process == "rhino-player" AND eventMessage BEGINSWITH "stall:"'` (in zsh, call `/usr/bin/log`, since `log` is a shell builtin).
- App holds: **`vf_swap_snap`** / **`vf_swap_unpause`** (`src/video_pref/vf_av_sync.rs`) carry a `Hold`. Reason tags are set at the call sites in `smooth_vf_chain_rebuild.rs` (`smooth-on reattach`, `smooth graph replace`, `smooth strip`) and `smooth_off_playhead_refresh.rs` (`audio-track switch`). The same-media reload for Smooth reattach (`smooth_vf_media_reload.rs`, `smooth reload`) keeps its `Hold` until the post-load unpause. `load-hold=true` in the snapshot means the busy-system hold (`smooth_load_hold_active`) triggered the strip.
- Trigger context: every dispatched `TransportEv` (Debug form, e.g. `VideoReconfig`, `ContainerFpsChanged`, `Pause(true)`) plus mpv `Seek` / `PlaybackRestart` / `AudioReconfig`, retained for `TRIGGER_WINDOW` (`src/stall_log.rs`).
- Buffering: mpv **`paused-for-cache`**, observed in `transport_observe_install.rs` (`PROP_PAUSED_FOR_CACHE`). Desync: mpv warn-level messages containing **`desynchronisation`**.
- Out of scope: load, browse, and chapter-scrub pauses, which are not mid-playback stalls.
