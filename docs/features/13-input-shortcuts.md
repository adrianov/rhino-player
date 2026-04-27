# Keyboard, mouse, and shortcuts

**Name:** Input and keyboard shortcuts

**Implementation status:** Done for in-app shortcuts in `src/app/input.rs` (not: forwarding arbitrary keys to mpv from user `input.conf`, Shortcuts help window)

**Use cases:** Power users keep mpv muscle memory; casual users can view or override keys; mouse maps match typical player expectations.

**Short description:** Forward keys to mpv with GTK accelerator conflicts avoided; custom `input.conf` in the config dir; built-in default bindings; optional Adwaita Shortcuts window listing effective bindings; mouse map for buttons and double-click; scroll maps to WHEEL keys.

**Long description:** Load internal bindings from a memory `input.conf`, then optional user `input.conf`. Key events in capture phase translate to mpv’s `keypress`/`keyup` with modifier and key remaps. Mouse button gestures send `keypress` for mapped `MBTN_*` commands. Scroll on overlay sends wheel key combos. Optional: `?` for shortcuts dialog populated from `input-bindings` property.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Keyboard and pointer input
  Scenario: App accelerators win over mpv forwarding
    Given a key combination is bound as a GTK application shortcut
    When the user presses it with the main window focused
    Then the application handles it and does not forward the same chord to mpv

  Scenario: Space with continue grid preloaded video
    Given the recent grid is visible and a first card is warm-preloaded per recent spec
    When the user presses Space
    Then the video is revealed and playback starts instead of staying behind the grid

  Scenario: Close Video without quitting
    Given a file with duration is loaded and the grid is not showing
    When the user activates Ctrl+W or Close Video
    Then playback stops and the continue grid appears without terminating the process

  Scenario: Escape twice returns to browse with pause-first semantics
    Given playback may be active or fullscreen per escape-first rule
    When the user presses Escape twice per documented sequencing
    Then audio stops promptly on first Esc where specified and navigation ends on recent grid when history exists

  Scenario: Trash shortcut eligibility
    Given a local file path is playing
    When the user presses Delete
    Then Move to Trash follows the trash feature rules
    And the action is inactive for streams or when the grid has focus per spec
```

- Do not pass keys that match registered app accelerators to mpv.
- **Space** toggles play/pause via the mpv `pause` property when the main window is focused (and the player is ready). If the continue grid is visible and the preloaded first item is ready, Space first reveals that video and starts playback instead of playing behind the grid.
- **Primary double-click** on the video view toggles fullscreen. **Secondary (right) single-click** on the video view toggles play/pause the same as Space (when a file with duration is loaded).
- **m** toggles mute (see [Audio volume / mute](22-audio-volume-mute.md)).
- **Up** / **Down** adjust volume by 5% (clamped).
- **q** and **Ctrl+Q** run `app.quit` (resume snapshot, then exit).
- **Ctrl+W** runs `app.close-video`: same as that post-fullscreen Escape path (return to the **continue / recent** grid and stop the current file) — **does not** quit the app. The **bottom** transport bar and main menu both expose **Close Video** for a mouse-only path. The action is disabled when the grid is already showing or the player is not ready.
- **Delete** and **KP_Delete** run `app.move-to-trash` when a **local file** is playing (not streams); same as the main menu **Move to Trash** (see [27-move-to-trash](27-move-to-trash.md)). Disabled on the continue grid or for non-file sources.
- **Enter** and **KP_Enter** toggle fullscreen (see [Window behavior](17-window-behavior.md)).
- On the **continue / recent** grid, **double** primary click on the **empty** area (padding above, below, or beside the card row, not on a card) toggles fullscreen the same as on the video (see [Recent videos](21-recent-videos-launch.md)).
- Escape leaves fullscreen first; a **second** Escape (when not fullscreen) **pauses** at once (so sound stops) while the app finishes resume/DB work and returns to the **recent-videos** card screen when there is history, then `stop` runs in the main-loop idle chain (same as empty launch; otherwise the overlay stays hidden and the status invites opening a file).
- Tab focuses UI chrome temporarily.
- Document location of `input.conf` under `~/.config/rhino/` (TBD), following XDG.
