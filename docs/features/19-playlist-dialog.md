# Playlist dialog (list, reorder, save m3u8)

**Name:** Playlist side dialog

**Implementation status:** Not started

**Use cases:** Reorder a queue, save it as a file for later, and jump to any item by sight—without hunting in the file system.

**Short description:** A dialog listing the current playlist with icons by MIME, playing row highlight, click to jump, drag-and-drop reorder (`playlist-move`), right-click: open in file manager, remove, drop to append; save to `.m3u8` with basic `#EXTINF` lines.

**Long description:** An `Adw` dialog (or sheet) builds rows from `mpv.playlist`. “Save playlist” must write paths that work when the app runs from a normal install and when the config/data dirs differ (resolve paths in a way that is portable for the user). DnD from the OS into the list appends. Reordering uses `playlist-move` with correct indices.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Playlist side dialog (target behavior — not started)
  Scenario: Follow active playlist row
    Given the playlist dialog is open and mpv advances playlist-pos
    When the position changes
    Then the list scrolls to keep the current row visible

  Scenario: Reorder with playlist-move
    Given multiple queued items
    When the user drags rows to a new order
    Then mpv playlist-move updates indices consistently with UI order

  Scenario: Save portable m3u8
    Given the user saves the playlist to disk
    When the operation completes
    Then paths and titles are written for reuse across installs per portability rules
```

- Scroll to current item on `playlist-pos` change.
- Right-click `open_containing_folder` for local files only.
- Save dialog filters `.m3u8` and writes paths/titles.
- DnD: append and refresh; show spinner while resolving large drops if needed.
