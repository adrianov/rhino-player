# Drag and drop

---
status: done
priority: p2
layers: [ui, playback]
related: [06, 07, 08, 24]
---

## Use cases
- Add files from the file manager or browser quickly.
- Add subtitles to the playing video with one drop.

## Description
The main window accepts drops of local paths from the system file list: the primary item replaces the current media using the normal open pipeline; additional playable paths append after that. While a clip is loaded, dropped subtitle files are added externally without replacing playback. Plain-text URL drops remain future work.

## Behavior

```gherkin
@status:done @priority:p2 @layer:ui @area:dnd
Feature: Drag and drop onto the video surface

  Scenario: Drop opens media
    Given the main window is visible
    When the user drops one or more local paths onto the window
    Then the first item replaces the current media through the usual open path
    And remaining playable items append to the playlist in drop order

  Scenario: Subtitle file added while playing
    Given a video is playing
    When the user drops a file whose extension is a known subtitle format
    Then an external subtitle resource is attached for that clip without replacing playback

  Scenario: Folder drop opens the directory for playback
    Given the user drops a directory
    When the drop completes
    Then the playback engine receives the folder through the usual open-file rules for directories
```

## Notes
- **Linux:** **`GtkDropTargetAsync`** on **`GtkApplicationWindow`** (`build_window/wire_drag_drop*.rs`). Primary payload: **`gdk_drop_read_async`** over MIME (**`text/uri-list`**, **`text/plain`**, **`x-special/gnome-copied-files`**, …); full stream drain then URI parse. Secondary: **`GdkDrop.read_value_async`** for **`GdkFileList`** / **`GFile`**.
- **macOS:** AppKit **`NSDraggingDestination`** on a transparent **`RhinoDropView`** subview of the window **`contentView`** (`macos_drag_drop.rs`) — gdk-macos GTK drop targets often miss Finder drops (especially when unfocused). **`hitTest:`** returns the view only while the drag pasteboard offers file URLs / filenames so normal GTK clicks pass through.
- Shared open: **`consume_dropped_paths`** → **`on_open`** (**`replace_media`**, **`play_on_start`**). Playlist tail: **`loadfile`** **`append`** on an idle. Subtitles: heuristic extensions while media is open → **`sub-add`**, then **`schedule_sub_button_scan`**.
- Remaining gaps: uncommon portal-only MIME blobs, URLs without local **`file:`** schemes, dedicated inline drop-error UI (`stderr` **`[rhino] dnd:`** only).
