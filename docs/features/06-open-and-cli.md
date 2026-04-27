# Open files: file picker, folder, CLI, single-instance

---
status: wip
priority: p1
layers: [ui, platform, mpv]
related: [05, 07, 11, 12, 21]
actions: [app.open]
---

## Use cases
- Open from the file manager, drag files in, or pass paths on the command line.
- Use one running window or several, depending on preference.

## Description
File dialogs open or add media; folders open as playlists; URL dialogs handle network sources. `GApplication`’s `open` receives external file lists and forwards them to the active window or a new one per preference. A `--new-window` flag exists for secondary instances when supported. On launch, the first `argv` path (if any) loads instead of showing the recent grid.

Today the **Open Video** dialog and CLI startup path are wired; drag-and-drop, single-instance policy, folder-as-playlist, and `HANDLES_OPEN` for remote activation are not.

## Behavior

```gherkin
@status:wip @priority:p1 @layer:platform @area:open
Feature: Open files and CLI integration

  Scenario: Open Video dialog lists video extensions only
    Given the user activates Open Video from the shell
    When the dialog is presented
    Then the listed extensions match the shared video suffix list
    And still-image and other unrelated types are excluded

  Scenario: Command-line startup loads first argv path
    Given the user launches the app with one or more argv paths
    When the first window paints with no conflicting session restore
    Then the first supported path loads instead of the recent grid

  Scenario: Secondary activation respects open-new-windows
    Given another instance activates with paths while open-new-windows is off
    When the running app receives those paths
    Then loads target the active window per documented replace-or-append rules

  Scenario: Folder argv loads first playable file
    Given the user passes a directory on the command line
    When the app resolves a playable file inside it
    Then that file loads via the standard load path
    And subsequent siblings follow the sibling-folder queue rules

  Scenario: Invalid CLI path falls back to the recent grid
    Given the user passes an unsupported or missing path
    When the app starts
    Then the recent grid is shown like an empty launch
    And the unsupported path is logged
```

## Notes
- The shared video suffix list lives in `src/video_ext.rs` and is reused by **Open Video** and sibling scanning.
- `--new-window` and `HANDLES_OPEN` (or the Rust equivalent) are planned but not shipped.
- Drag-and-drop is owned by [11-drag-and-drop](11-drag-and-drop.md); URL input by [12-url-and-streams](12-url-and-streams.md).
