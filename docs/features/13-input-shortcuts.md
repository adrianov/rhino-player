# Keyboard, mouse, and shortcuts

---
status: done
priority: p1
layers: [input, ui, mpv]
related: [02, 17, 21, 22, 27]
actions: [app.quit, app.open, app.close-video, app.move-to-trash, app.exit-after-current]
---

## Use cases
- Power users keep mpv muscle memory.
- Casual users get familiar shortcuts (Space, Escape, arrows).
- Mouse maps match typical player expectations.

## Description
GTK accelerators handle window-scope shortcuts in capture phase. Application accelerators are not forwarded to mpv to avoid double-handling. Mouse maps cover primary double-click (toggle fullscreen), right-click (toggle pause), and scroll on the video surface (volume).

## Behavior

```gherkin
@status:done @priority:p1 @layer:input @area:shortcuts
Feature: Keyboard and pointer input

  Scenario: App accelerators are not forwarded to mpv
    Given a key combination is bound as a GTK application shortcut
    When the user presses it with the main window focused
    Then the application handles it
    And the same chord is not also delivered to mpv

  Scenario: Space toggles play / pause when ready
    Given the main window is focused and a file with duration is loaded
    When the user presses Space
    Then mpv pause toggles
    And no extra notification is shown

  Scenario: Space reveals warm-preloaded continue card
    Given the recent grid is visible and the first card is warm-preloaded
    When the user presses Space
    Then the video is revealed and playback starts
    And playback does not start hidden behind the grid

  Scenario: Ctrl+W returns to browse without quitting
    Given a file with duration is loaded and the grid is hidden
    When the user activates Ctrl+W or Close Video
    Then playback stops
    And the continue / recent grid appears
    And the application process keeps running

  Scenario: Escape pauses then returns to grid
    Given playback is active and the app is windowed
    When the user presses Escape twice per the documented sequence
    Then the first Escape pauses (so audio stops promptly)
    And the second Escape returns to the recent grid via the idle stop chain when history exists

  Scenario: Delete moves a local file to trash
    Given a local regular file is playing and the grid is hidden
    When the user presses Delete or KP_Delete
    Then app.move-to-trash runs per 27-move-to-trash
    And streams or grid focus leave the action disabled

  Scenario: Volume keys nudge by 5%
    Given the player is ready
    When the user presses Up or Down
    Then volume changes by 5%, clamped to volume-max
    And no extra notification is shown

  Scenario: Mute toggle on m
    Given the player is ready
    When the user presses m
    Then mute toggles like the popover toggle in 22-audio-volume-mute

  Scenario: Quit on q or Ctrl+Q
    Given the main window is open
    When the user presses q or Ctrl+Q
    Then app.quit writes resume snapshot
    And the application exits

  Scenario: Enter toggles fullscreen
    Given the main window is focused
    When the user presses Enter or KP_Enter
    Then fullscreen toggles like double-click on the video surface

  Scenario: Right click toggles play / pause
    Given a file with duration is loaded and the grid is hidden
    When the user right-clicks on the video surface
    Then mpv pause toggles like Space
```

## Notes
- Default bindings load from a memory `input.conf`; an optional user `input.conf` under `~/.config/rhino/` is reserved for later (TBD).
- Empty-area double-click on the recent grid spacers also toggles fullscreen (see [21-recent-videos-launch](21-recent-videos-launch.md)).
- Tab focuses chrome temporarily.
