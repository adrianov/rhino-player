# Chapters: marks, menu, seek bar hover

---
status: wip
priority: p2
layers: [ui, playback]
related: [04, 18]
---

## Use cases
- Navigate DVD-like or long-form content by chapter.
- See chapter starts on the timeline.
- Jump from a list or directly from the seek bar.

## Description
When the playback engine exposes chapter metadata for the open title, the seek bar draws a vertical mark at each chapter start and shows the chapter name in the seek bar tooltip when the pointer is close to that position. A main-menu chapter list remains planned.

## Behavior

```gherkin
@status:wip @priority:p2 @layer:playback @area:chapters
Feature: Chapters: marks, menu, seek bar hover

  Scenario: Chapter marks render at chapter starts
    Given the playback engine exposes a non-empty chapter list for the current title
    When the seek bar is shown
    Then a mark appears at every chapter start time in ascending order

  Scenario: Empty chapter list hides chapter marks
    Given the chapter metadata lists no chapters for the current title
    When the seek bar is shown
    Then no chapter marks render on the seek bar

  Scenario: Seek bar tooltip shows chapter name near a mark
    Given chapters exist for the current file
    When the pointer hovers the seek bar close to a chapter start
    Then the seek bar shows a tooltip with that chapter name
```

## Notes
- **Shipped:** `chapter-list` via libmpv `MPV_FORMAT_NODE`: array of maps with `time` (double) and `title` (string). Parsed in `chapter_list::mpv_chapter_list`; UI refreshed from transport **`FileLoaded`**, **`PathChanged`**, **`Duration`**, **`VideoReconfig`**. Marks: **`GtkDrawingArea`** overlay on **`GtkOverlay`** wrapping **`Gtk.Scale`** (`rp-seek-chapters`), **`GtkDrawingAreaExtManual::set_draw_func`** for Cairo ticks; **`EventControllerMotion`** on the scale updates **`gtk::Scale::set_tooltip_text`** near each chapter time (`seek_chapter_ui.rs`). **`libmpv2-sys`** is pulled for `mpv_node` FFI alongside **`libmpv2`**.
- **Planned:** main-menu chapter actions (`mpv` **`chapter`** property / numbered seeks); richer hover may align with [18-thumbnail-preview](18-thumbnail-preview.md).
