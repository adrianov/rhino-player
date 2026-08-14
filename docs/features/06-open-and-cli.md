# Open files: file picker, folder, CLI, single-instance

---
status: wip
priority: p1
layers: [ui, os-integration, playback]
related: [07, 11, 12, 21]
---

## Use cases
- Open from the file manager, drag files in, or pass paths on the command line.
- Use one running window or several, depending on preference.

## Description
File dialogs open or add media; folders follow the same sibling-folder rules as in-product navigation (see [07-sibling-folder-queue](07-sibling-folder-queue.md)); URL dialogs handle network sources. `GApplication`’s `open` receives external file lists and forwards them to the active window or a new one per preference. A `--new-window` flag exists for secondary instances when supported. On launch, the first `argv` path (if any) loads instead of showing the recent grid.

Today the **Open Video** dialog, CLI startup path, and [drag-and-drop](11-drag-and-drop.md) are wired; single-instance policy, full folder-open behaviour, and `HANDLES_OPEN` for remote activation are not.

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

  Scenario: File manager offers Rhino for MPEG and VOB files
    Given Rhino Player is installed as a desktop application bundle
    When the user inspects an MPEG program-stream or DVD VOB file in the file manager
    Then Rhino appears among applications that can open that item

  Scenario: File manager offers Rhino for in-progress Direct Connect downloads
    Given Rhino Player is installed as a desktop application bundle
    When the user inspects a local file whose name ends with the in-progress download suffix used by Direct Connect clients
    Then Rhino appears among applications that can open that item
    And the Open Video filter includes that suffix in the shared video suffix list

  Scenario: Open Video accepts a DVD disc folder
    Given the user activates Open Video with the video file filter
    When the user selects a directory that contains a valid disc index for DVD
    Then playback starts from the first title chapter in that tree
    And further chapters in the same folder are reachable via sibling navigation

  Scenario: Video file beside a disc index loads as that file
    Given a directory contains both a local video file and a disc index
    When the user opens the video file
    Then that file loads
    And the disc does not load in its place

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

  Scenario: Hollow or zero-filled local file stays on the continue grid
    Given the user opens a local video path whose bytes are missing or all zeroes
    When the open pipeline runs preflight or the playback engine reports an unrecognized file
    Then the continue grid remains visible (or is restored)
    And a notice toast explains that the file looks empty or unfinished
    And the path is not kept as a playable continue entry

  Scenario: Unreadable media shows an open-failure notice
    Given the user opens a local file that is not demuxable
    When mpv ends the load with an error
    Then a notice toast reports that Rhino could not read media from the file
    And playback does not remain on a blank video surface
```

## Notes
- Open failures (empty/hollow files, demux errors, missing paths) surface a continue-grid notice toast (`src/media_open_fail.rs`, `NoticeToast`) and return to browse when playback was entered. Zero-filled torrent preallocation is detected before `loadfile`.
- Shared suffixes: `src/video_ext/` ([SUFFIX], reused by **Open Video** and sibling scan). **`dctmp`**: in-progress Direct Connect download (often `name.mkv.<id>.dctmp`) — not a hollow zero-filled stub. Disc trees: `OpticalDisc` + `VideoTsDir` (**BDMV** → disc root; **VIDEO_TS** → `dvd_first_playable_vob`; many engines lack `dvd://`). Files inside `VIDEO_TS/` belong to that DVD; a neighbouring `VIDEO_TS` does not divert `.mkv`/`.mp4` opens (`OpticalDisc::dvd_root`). macOS open panel: `macos_open_video.rs`; Finder: `Info.plist.in` (incl. **`.dctmp`**). Linux: desktop / AppStream; **`.dctmp`** → `application/x-dcpp-incomplete` (`data/mime/packages/`, installed by user/system/deb scripts).
- External open while a window is up: `connect_open` in `src/app/base/preload_continue_and_run.rs` queues `on_open` on a one-shot GTK idle (never synchronous `try_load` in the signal — macOS re-entrancy / `RefCell` abort). `load_file_into_player` uses `try_borrow_mut` like transport drain.
- `--new-window` and `HANDLES_OPEN` (or the Rust equivalent) are planned but not shipped.
- Drag-and-drop is owned by [11-drag-and-drop](11-drag-and-drop.md); URL input by [12-url-and-streams](12-url-and-streams.md).
