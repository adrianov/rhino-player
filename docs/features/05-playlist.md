# Playlist: queue, prev/next, shuffle, loop

---
status: planned
priority: p2
layers: [ui, mpv]
related: [04, 07, 16, 19]
mpv_props: [playlist-pos, playlist-count, playlist, playlist-shuffle, loop-file, loop-playlist, eof-reached]
---

## Use cases
- Binge a series in order without re-opening files.
- Shuffle music or loop a single tutorial.
- Keep playback after EOF per `keep-open`.

## Description
mpv’s playlist is the source of truth. The UI exposes shuffle and loop toggles, enables previous / next when navigation is meaningful, and at the end of a single file with `keep-open` may rewind and pause. Replace vs append-play semantics live in [open](06-open-and-cli.md), [drag and drop](11-drag-and-drop.md), and [URL streams](12-url-and-streams.md).

## Behavior

```gherkin
@status:planned @priority:p2 @layer:mpv @area:playlist
Feature: Playlist navigation

  Scenario: Shuffle changes playback order
    Given multiple items are queued in the mpv playlist
    When the user enables shuffle from the playlist UI
    Then subsequent next requests visit items in shuffled order
    And playlist-count remains correct

  Scenario: Previous wraps when more than one item exists
    Given wrap behaviour is enabled and the queue has at least two items
    When the user activates previous at the first item
    Then playback moves to the last item

  Scenario: Loop modes are mutually exclusive in UI
    Given the user enables loop-file
    When they then enable loop-playlist
    Then loop-file is cleared and only loop-playlist remains active

  Scenario: Buttons disable without a target
    Given the active item has no previous or next neighbour and no wrap
    When the user inspects the transport bar
    Then the corresponding navigation button is disabled
```

## Notes
- Drive button sensitivity from `playlist-pos`, `playlist-count`, and the `playlist` array.
- `eof-reached` and idle handling coordinate with [16-session-persistence](16-session-persistence.md) and window close behaviour.
- The full list / reorder UI is owned by [19-playlist-dialog](19-playlist-dialog.md); this feature is only the queue mechanics + transport buttons.
