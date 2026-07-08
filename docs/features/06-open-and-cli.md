# Open files: file picker, folder, CLI, single-instance

---
status: wip
priority: p1
layers: [ui, platform, mpv]
related: [07, 11, 12, 21]
actions: [app.open]
---

## Use cases
- Open from the file manager, drag files in, or pass paths on the command line.
- Use one running window or several, depending on preference.

## Description
File dialogs open or add media; folders follow the same sibling-folder rules as in-product navigation (see [07-sibling-folder-queue](07-sibling-folder-queue.md)); URL dialogs handle network sources. `GApplication`’s `open` receives external file lists and forwards them to the active window or a new one per preference. A `--new-window` flag exists for secondary instances when supported. On launch, the first `argv` path (if any) loads instead of showing the recent grid.

Today the **Open Video** dialog and CLI startup path are wired; drag-and-drop, single-instance policy, full folder-open behaviour, and `HANDLES_OPEN` for remote activation are not.

## Behavior

```gherkin
@status:wip @priority:p1 @layer:platform @area:open
Feature: Open files and CLI integration

  Scenario: Open Video dialog lists video extensions only
    Given the user activates Open Video from the shell
    When the dialog is presented
    Then the listed extensions match the shared video suffix list
    And still-image and other unrelated types are excluded

  Scenario: Open Video accepts a Blu-ray disc folder
    Given the user activates Open Video with the video file filter
    When the user selects a directory that contains a valid disc index for Blu-ray or AVCHD
    Then that disc loads through the standard open path
    And sibling-folder navigation does not treat the disc as a normal video file in a folder

  Scenario: File manager offers Rhino for Blu-ray disc packages
    Given Rhino Player is installed as a desktop application bundle
    When the user inspects a Blu-ray or AVCHD disc package in the file manager
    Then Rhino appears among applications that can open that item

  Scenario: Open Video accepts a DVD disc folder
    Given the user activates Open Video with the video file filter
    When the user selects a directory that contains a valid disc index for DVD
    Then playback starts from the first title chapter in that tree
    And further chapters in the same folder are reachable via sibling navigation

  Scenario: Command-line startup loads first argv path
    Given the user launches the app with one or more argv paths
    When the first window paints with no conflicting session restore
    Then the first supported path loads instead of the recent grid

  Scenario: Secondary activation respects open-new-windows
    Given another instance activates with paths while open-new-windows is off
    When the running app receives those paths
    Then loads target the active window per documented replace-or-append rules

  Scenario: File manager opens media while playback is active
    Given the app is playing media in the main window
    When the file manager sends an open request for another supported file
    Then the new file loads without crashing
    And the window comes to the foreground

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
- The shared video suffix list lives in `src/video_ext.rs` and is reused by **Open Video** and sibling scanning. **BDMV** / AVCHD: `bluray_disc_root` → `loadfile` on disc root. **DVD** `VIDEO_TS`: `dvd_disc_root`, then `dvd_first_playable_vob` (first `VTS_*_1.VOB` in `VIDEO_TS/`) because many mpv builds lack `dvd://`. **Prev/Next** walks other `.vob` files in `VIDEO_TS/`. macOS **Open Video**: `src/macos_open_video.rs` (`NSOpenPanel` + `setAllowedContentTypes`). macOS **Finder** “Open With”: `packaging/macos/Info.plist.in` declares `public.avchd-content` / `public.avchd-collection` with `LSTypeIsPackage` and `.bdmv` / `.bdm` / `.avchd` extensions (same UTIs as the open panel). Linux: GTK `FileDialog`.
- External open while a window is up: `connect_open` in `src/app/base/preload_continue_and_run.rs` queues `on_open` on a one-shot GTK idle (never synchronous `try_load` in the signal — macOS re-entrancy / `RefCell` abort). `load_file_into_player` uses `try_borrow_mut` like transport drain.
- `--new-window` and `HANDLES_OPEN` (or the Rust equivalent) are planned but not shipped.
- Drag-and-drop is owned by [11-drag-and-drop](11-drag-and-drop.md); URL input by [12-url-and-streams](12-url-and-streams.md).
