# Chapters: marks, menu, seek bar hover

**Name:** Chapters UI

**Implementation status:** Not started

**Use cases:** Navigate DVD-like or long-form content by chapter; see where chapters start on the timeline; jump from a list or the seek bar.

**Short description:** Chapter marks on the seek bar, a chapter list menu, and a popover on hover over the bar showing time and chapter title.

**Long description:** When `chapter-list` is non-empty, draw marks at chapter start times, populate a `Gio.Menu` with `chapter` index actions, and show a small popover following the pointer on the bar with optional thumbnail (if preview enabled). Selecting a chapter uses mpv’s `chapter` index property.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Chapters (target behavior — not started)
  Scenario: Chapter marks with data
    Given mpv exposes a non-empty chapter-list
    When the seek bar is shown
    Then chapter start times are reflected as marks at sorted positions on the scale

  Scenario: Empty chapters hide UI affordances
    Given chapter-list is empty or unavailable
    When the window is visible
    Then chapter-specific marks and menus stay hidden or cleared

  Scenario: Seek by chapter index
    Given chapters exist and the user selects a chapter from the menu
    When the action activates
    Then mpv chapter property updates and playback jumps to that chapter start
```

- Sort chapters by `time`.
- `select-chapter` state syncs with `chapter` property.
- If no chapters, hide the chapters menu and clear marks.
- Popover: escape title text for markup; position relative to the scale widget.
