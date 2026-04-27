# Chapters: marks, menu, seek bar hover

---
status: planned
priority: p2
layers: [ui, mpv]
related: [04, 18]
mpv_props: [chapter, chapter-list]
---

## Use cases
- Navigate DVD-like or long-form content by chapter.
- See chapter starts on the timeline.
- Jump from a list or directly from the seek bar.

## Description
When `chapter-list` is non-empty, the seek bar shows marks at chapter starts, the main menu lists chapters as actions, and a popover near the pointer shows time and chapter title on hover. Selecting a chapter sets mpv’s `chapter` property.

## Behavior

```gherkin
@status:planned @priority:p2 @layer:mpv @area:chapters
Feature: Chapter navigation

  Scenario: Chapter marks render at chapter starts
    Given mpv exposes a non-empty chapter-list
    When the seek bar is shown
    Then a mark appears at every chapter start time, sorted by time

  Scenario: Empty chapter-list hides chapter UI
    Given chapter-list is empty or unavailable
    When the user inspects the seek bar and main menu
    Then no chapter marks render and the chapter menu is hidden

  Scenario: Selecting a chapter seeks to its start
    Given chapters exist for the current file
    When the user activates a chapter entry in the menu
    Then playback jumps to that chapter’s start time
    And the mpv chapter property reflects the selected index

  Scenario: Hover popover shows chapter title and time
    Given the pointer hovers a position inside a chapter on the seek bar
    When the popover is visible
    Then the popover shows the formatted hover time and the chapter title
```

## Notes
- Escape chapter title text for markup; position the popover relative to the scale widget.
- May share its hover popover with [18-thumbnail-preview](18-thumbnail-preview.md).
