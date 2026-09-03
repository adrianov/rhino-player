# Keyboard, mouse, and shortcuts

---
status: done
priority: p1
layers: [input, ui, playback]
related: [02, 07, 17, 21, 22, 27, 28, 33]
---

## Use cases
- Power users keep familiar media-player muscle memory.
- Casual users get familiar shortcuts (Space, Escape, arrows).
- Mouse maps match typical player expectations.

## Description
Window-scope shortcuts run in a capture-phase key controller so focused chrome does not steal playback chords. Application accelerators are not also forwarded to the playback engine (avoids double-handling). Mouse maps cover primary double-click (toggle fullscreen), right-click (toggle pause), and scroll on the video surface (volume). **Enter**, **KP_Enter**, **f**, and **F** share one fullscreen toggle like typical players. **Escape** returns to the continue grid during playback; a further **Escape** on the continue grid quits the app. It does **not** exit fullscreen (use Enter, **f**, **F**, or double-click for that).

## Behavior

```gherkin
@status:done @priority:p1 @layer:input @area:shortcuts
Feature: Keyboard and pointer input

  Scenario: Application shortcuts are not double-handled by the engine
    Given a key combination is bound as an application shortcut
    When the user presses it with the main window focused
    Then the application handles it
    And the same chord is not also delivered to the playback engine

  Scenario: Space toggles play / pause when ready
    Given the main window is focused and a file with duration is loaded
    When the user presses Space
    Then pause toggles
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

  Scenario: Escape returns to continue grid during playback
    Given playback is active and the continue grid is hidden
    When the user presses Escape once
    Then playback pauses promptly
    And the continue grid appears via the browse-back path when history supports it

  Scenario: Escape on continue grid quits
    Given the continue grid is visible
    And focus is not in a text entry
    When the user presses Escape
    Then a resume snapshot is written
    And the application exits

  Scenario: Escape shows continue grid without leaving fullscreen
    Given playback is active and the continue grid is hidden
    And the viewing layout uses fullscreen presentation
    When the user presses Escape once
    Then playback pauses promptly
    And the continue grid appears via the browse-back path when history supports it
    And the viewing layout stays fullscreen until the user exits fullscreen another way

  Scenario: Delete moves a local file to trash
    Given a local regular file is playing and the grid is hidden
    When the user presses Delete or KP_Delete
    Then the open local file is moved to the platform trash per 27-move-to-trash
    And streams or grid focus leave the action disabled

  Scenario: Ctrl with arrows jumps previous / next sibling
    Given a file with duration is loaded and the continue grid is hidden
    When the user presses Ctrl+Left or Ctrl+Right (including keypad arrows with Ctrl)
    Then the previous or next sibling in folder order loads like the bottom-bar buttons

  Scenario: Arrow keys step playback by five seconds
    Given a file with duration is loaded and the continue grid is hidden
    When the user presses Left or Right (including keypad arrows)
    Then playback position moves backward or forward by five seconds respectively
    And the position stays within the beginning and end of the media

  Scenario: Volume keys nudge by 5%
    Given the player is ready
    When the user presses Up or Down
    Then volume changes by 5%, clamped to the configured maximum
    And no extra notification is shown

  Scenario: Mute toggle on m
    Given the player is ready
    When the user presses m
    Then mute toggles like the popover toggle in 22-audio-volume-mute

  Scenario: Digit one through eight sets playback rate
    Given the main window is focused and a session is ready to control playback
    When the user presses a single digit between one and eight on the main keyboard or matching keypad keys
    Then playback rate matches the fixed-rate shortcut mapped to that digit
    And the speed control highlight matches the chosen rate on the canonical step list

  Scenario: Copy open media as a filesystem item
    Given a local file or disc shell path is open for playback
    When the user presses the platform copy chord with the main window focused
    And focus is not in a text entry that needs raw keys
    Then the open item is placed on the system clipboard as a filesystem item
    And a paste in the platform file manager creates a copy of that item
    And no extra notification is shown

  Scenario: Quit on q or Ctrl+Q
    Given the main window is open
    When the user presses q or Ctrl+Q
    Then a resume snapshot is written
    And the application exits

  Scenario: Typing q in the neighbour search box does not quit
    Given the continue screen search box has focus
    When the user types the letter q
    Then the letter appears in the search box
    And the application keeps running

  Scenario: Enter or f toggles fullscreen
    Given the main window is focused
    When the user presses Enter, KP_Enter, f, or F
    Then fullscreen toggles like double-click on the video surface

  Scenario: Right click toggles play / pause
    Given a file with duration is loaded and the grid is hidden
    When the user right-clicks on the video surface
    Then pause toggles like Space

  Scenario: Dedicated play and pause media controls match Space
    Given a file with duration is loaded or the first continue card is warm-preloaded
    When the user activates the host play or pause media control with the main window focused
    Then pause toggles or the warm card reveals like Space

  Scenario: Dedicated stop media control pauses
    Given a file with duration is loaded and the continue grid is hidden
    When the user activates the host stop media control with the main window focused
    Then playback pauses

  Scenario: Dedicated previous and next media controls load siblings
    Given a file with duration is loaded and the continue grid is hidden
    When the user activates the host previous-track or next-track media control with the main window focused
    Then the previous or next sibling in folder order loads like Ctrl with arrow
```

## Notes
- GIO actions wired for this feature: `app.quit`, `app.open`, `app.close-video`, `app.move-to-trash`, `app.exit-after-current`.
- Default bindings load from a memory `input.conf`; an optional user `input.conf` under `~/.config/rhino/` is reserved for later (TBD).
- Empty-area double-click on the recent grid spacers also toggles fullscreen (see [21-recent-videos-launch](21-recent-videos-launch.md)). Double-click primary on the top toolbar exits fullscreen anytime, or enters fullscreen during playback while the overlay is hidden (same rules as GL double-click).
- **f** / **F** toggles fullscreen like Enter or KP_Enter.
- **Escape** opens the continue grid (browse-back) during playback; when the grid is already visible and focus is not in a text entry, it activates `app.close-video` (which quits on browse — `key_escape_seek.rs`). It does **not** exit fullscreen.
- Tab focuses chrome temporarily.
- Arrow Left / Right (and keypad arrows) step **playback position** five seconds when the seek bar is enabled and the continue grid is hidden; implementation shares the transport seek path ([04-transport-and-progress](04-transport-and-progress.md)).
- Ctrl+Left / Ctrl+Right load the previous / next sibling file like the bottom bar ([07-sibling-folder-queue](07-sibling-folder-queue.md)).
- Hardware **play**, **pause**, **stop**, **previous**, and **next** keys (GDK `AudioPlay`, `AudioPause`, `AudioStop`, `AudioPrev`, `AudioNext`) are handled in the same capture-phase controller **when the main window is focused**; behaviour matches Space and Ctrl+arrows as above. True background / unfocused routing is OS-specific (on macOS that may require separate Now Playing integration).
- Digit **1**–**8** (and keypad **KP_1**–**KP_8**), **without** Ctrl / Alt / Meta / Super (see `DIGIT_SPEED_BLOCK` in `input/digit_speed_keys.rs`): **3** → **1.5×**, other digits → **N**× (`input/digit_speed_keys.rs` → `playback_speed`, same idle resync path as header list in **28-playback-speed**). Keys are ignored when media is unavailable (capture handler exits before mutating mpv).
- **Copy file:** **macOS** **⌘C** / **Linux** **Ctrl+C** in the same capture-phase controller (`input/copy_playing_path.rs` → `shell_media_path` on mpv + `me_budget_shell_path`). Puts a file-manager item on the clipboard (**macOS** `NSPasteboard` `writeObjects:` with a file or directory `NSURL`, after releasing any GTK clipboard owner; **Linux** `GdkFileList` / `text/uri-list`), not plain path text — paste in Finder / Nautilus copies the file or disc folder. Skipped when focus is in an entry / text view so normal text copy still works. No toast.
- **q** quits via the capture-phase controller (`input/keys.rs` → `quit_key`) after the editable-focus guard (`root_focus_wants_raw_keys` walks ancestors — SearchEntry focuses an inner Editable `Text`). Cmd/Ctrl+Q stays on `app.quit` accelerators (`final_actions_wire`); plain `q` is never registered as an accel so GTK cannot quit behind the guard.
