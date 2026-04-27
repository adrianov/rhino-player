# Subtitles: style, track picker, auto-pick

---
status: done
priority: p1
layers: [ui, mpv, db]
related: [08, 14, 17]
settings: [sub_track_label, sub_off, sub_color, sub_border_color, sub_border_size, sub_scale]
mpv_props: [track-list, sid, sub-visibility, sub-color, sub-border-color, sub-border-size, sub-scale, sub-pos, sub-ass-override]
---

## Use cases
- Watch with readable on-screen text.
- Pick the right subtitle track when a file has many.
- Have the next file auto-pick the closest match to your last hand-picked track.

## Description
mpv `sub-*` defaults match a warm theatrical look (yellow text, dark outline, legible scale). A header **Subtitles** `MenuButton` is hidden until `track-list` exposes at least one subtitle stream. When shown, the popover offers a scrollable track list (with **Off**), a `sub-scale` control, and a text colour control. Preferences (last hand-picked label, persistent **Off**, colour, scale) live in SQLite and reapply after each load.

After each successful `loadfile`, the app re-applies styling, then runs auto-pick by normalised Levenshtein distance against the last hand-picked label (or a short `LANG` hint before any manual pick), unless the user is in persistent **Off** mode.

## Behavior

```gherkin
@status:done @priority:p1 @layer:mpv @area:subtitles
Feature: Subtitles styling and selection

  Scenario: Header button appears only when subs exist
    Given track-list eventually contains at least one subtitle stream for the loaded file
    When the bounded scan completes
    Then the Subtitles MenuButton becomes visible with track rows and styling controls

  Scenario: No subtitle tracks keeps the button hidden
    Given the loaded file has no subtitle streams after the bounded scan
    When the user inspects the header
    Then the Subtitles MenuButton remains hidden
    And no empty popover can be opened

  Scenario: Manual pick persists and clears Off mode
    Given the user selects a subtitle row from the popover
    When mpv applies that track
    Then SQLite stores the chosen label as sub_track_label
    And sub_off is set to false

  Scenario: Off persists across files until the user picks a track
    Given the user selected Off in a previous session
    When new files load
    Then sub-visibility stays off and auto-pick does not run
    And selecting any real subtitle row clears sub_off

  Scenario: Auto-pick chooses the closest subtitle track
    Given sub_off is false and a saved sub_track_label exists
    When auto-pick runs after styling is reapplied
    Then sid points to the track with the highest normalised Levenshtein score against the saved label
    And ties resolve by track id ascending

  Scenario: Auto-pick falls back to LANG before any manual pick
    Given sub_off is false and no sub_track_label is stored
    When auto-pick runs
    Then sid is selected by best Levenshtein match against the user’s LANG hint above a similarity floor
    And no track is chosen if no candidate clears the floor

  Scenario: Subtitles stay above the bottom toolbar
    Given the bottom ToolbarView is revealed
    When chrome is visible
    Then sub-pos is raised so subtitles render above the seek/times row
    And sub-pos returns to default when chrome auto-hides
```

## Notes
- `sub-color` / `sub-border-color` are passed as `#RRGGBB` strings (libmpv ignores int forms here).
- `sub-ass-override=force` makes ASS subs follow Rhino’s style overrides.
- Errors from setting sub properties are logged only; no UI notification.
