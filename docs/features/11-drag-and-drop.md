# Drag and drop

---
status: planned
priority: p2
layers: [ui, mpv]
related: [05, 06, 08, 24]
---

## Use cases
- Add files from the file manager or browser quickly.
- Add subtitles to the playing video with one drop.

## Description
The main video area accepts drops of files, folders, and text URLs. The first dropped item replaces the current media, additional items append. While playback is active, dropped subtitle files route to `sub-add` instead of replacing media.

## Behavior

```gherkin
@status:planned @priority:p2 @layer:ui @area:dnd
Feature: Drag and drop onto the video surface

  Scenario: Drop opens media
    Given the main window is visible
    When the user drops one or more playable media paths onto the video area
    Then the first item replaces the current media via loadfile
    And remaining items append in drop order

  Scenario: Subtitle file added while playing
    Given a video is playing
    When the user drops a file whose extension is a known subtitle format
    Then sub-add loads the file as a subtitle track
    And the dropped subtitle becomes selected

  Scenario: Folder drop loads the directory as a playlist
    Given the user drops a directory
    When the drop completes
    Then mpv loadfile receives the directory path
```

## Notes
- Drop targets accept GTK file lists and plain URL strings.
- Use a heuristic for local path vs URL before deciding load mode.
- Show a non-intrusive error inline when drops fail; do not introduce new toasts.
