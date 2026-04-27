# Playlist dialog (list, reorder, save m3u8)

---
status: planned
priority: p2
layers: [ui, mpv]
related: [05, 06, 12, 16]
mpv_props: [playlist, playlist-pos]
---

## Use cases
- Reorder a queue, save it as a file for later, jump to any item by sight.
- Avoid hunting in the file system to manage what is playing.

## Description
An Adwaita dialog (or sheet) lists the current playlist with MIME icons, highlights the playing row, supports click-to-jump, drag-and-drop reorder via mpv `playlist-move`, right-click open / remove, and saves portable `.m3u8` with `#EXTINF` lines.

## Behavior

```gherkin
@status:planned @priority:p2 @layer:ui @area:playlist-dialog
Feature: Playlist side dialog

  Scenario: Active row stays visible
    Given the playlist dialog is open and mpv playlist-pos advances
    When the position changes
    Then the list scrolls to keep the playing row visible

  Scenario: Reorder via drag-and-drop maps to playlist-move
    Given multiple items are queued
    When the user drags a row to a new position
    Then mpv playlist-move runs with the matching from / to indices
    And the rendered order matches mpv playlist after the move

  Scenario: Save as portable m3u8
    Given a non-empty playlist
    When the user saves the playlist to disk
    Then a valid m3u8 file is written with one path per item and EXTINF lines

  Scenario: Right-click on local file offers open in file manager
    Given the user right-clicks on a row whose path is a local file
    When the context menu is shown
    Then it offers Open Containing Folder
    And remote URLs do not show that entry
```

## Notes
- DnD from the OS appends; show a spinner while resolving large drops if needed.
- File save dialog filters `.m3u8`.
