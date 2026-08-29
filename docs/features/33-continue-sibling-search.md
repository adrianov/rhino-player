# Sibling search on the continue screen

---
status: done
priority: p1
layers: [ui, storage, persistence]
related: [07, 21, 34]
---

## Use cases
- Find the next episode sitting next to something already known to the player without going back through **Open Video**.
- Jump straight to any neighbouring video whose file name contains a known fragment (episode number, title word, release tag), including files already on the continue list.
- Reach videos in a folder next to a known title’s folder (same parent), without scanning the whole disk or other top-level libraries.

## Description
The browse screen grows a **search box above the video card strip**. Once per session the player builds a neighbour index from every path in the media files catalog: it lists video files in each known file’s folder and in that folder’s sibling folders (folders that share the same parent). It never lists the filesystem root and never treats top-level folders as siblings of each other (for example it will not walk from one library root to another under `/`). Typing filters that index by file-name substring (letter case ignored) and shows matches as regular cards in the same horizontal strip. The strip switches between plain watch-later cards and search-result cards in place — no navigation, no extra screen.

Result cards open and warm-preload like continue cards; they omit Remove / Move-to-Trash on the search strip (list management stays on the plain continue view). While a query is active the strip shows only results; clearing the box (or pressing Escape while typing) restores the plain continue list. The strip and Open Video tile stay put while typing; cards update only after filtering settles.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui @area:sibling-search
Feature: Sibling search on the continue screen

  Background:
    Given the continue screen is visible with its Open Video tile and search box

  Scenario: Search box sits centered above the card strip
    Given the first window is shown with no CLI paths and no session takeover
    When the window paints
    Then a search box is visible centered horizontally
    And the search box sits just above the video card strip
    And the strip below still shows the Open Video tile plus watch-later cards

  Scenario: Empty query keeps the plain continue strip
    Given the search box contains no text
    When the card strip paints
    Then the strip shows the Open Video tile followed by the usual watch-later cards

  Scenario: Typing keeps the strip stable until filtering settles
    Given the continue screen shows the plain watch-later cards
    When the user types a search fragment without pausing long enough for filtering to finish
    Then the strip still shows the same cards it showed before typing
    And the Open Video tile does not disappear or rebuild

  Scenario: Settled typing swaps the strip to matching neighbour cards
    Given the media files catalog holds a path inside a folder that also holds other video files
    When the user types a fragment of one of those neighbour file names and filtering finishes
    Then the strip replaces the watch-later cards with one card per matching file
    And the Open Video tile remains the first tile without flashing away
    And each card carries that file's title and progress from the store when known

  Scenario: Sibling folders of a known folder are searchable
    Given the catalog holds a video under one show folder
    And another show folder sits beside it under the same non-root parent
    And that sibling folder holds a differently named video
    When the user types a fragment of the sibling folder's video name and filtering finishes
    Then that sibling video appears among the result cards

  Scenario: Top-level library roots are not treated as siblings
    Given the catalog holds a video under one top-level library folder
    And another top-level library folder exists beside it under the filesystem root
    When the user searches
    Then videos under that other top-level library do not appear solely because the folders share the root
    And the filesystem root itself is never scanned for videos

  Scenario: Neighbour index builds once per session
    Given the neighbour index has already been built this session
    When the user changes the search fragment and filtering finishes again
    Then matching uses the same neighbour index without scanning folders again

  Scenario: Matching is case-insensitive and substring-based
    Given neighbour folders contain video files with mixed-case names
    When the user types a fragment differing only in letter case
    Then every neighbour whose full file name contains that fragment ignoring case appears
    And neighbours without the fragment do not appear

  Scenario: Continue-list files remain searchable
    Given a matching file is already on the watch-later list
    When the user searches with a fragment that matches it
    Then that file appears among the result cards

  Scenario: Result cards open and warm-preload without list controls
    Given the strip shows search-result cards
    When the user rests the pointer on a result card
    Then the file warm-preloads paused behind the grid like a hovered continue card
    And the card shows no Remove and no Move to Trash controls
    When the user clicks the result card
    Then that file loads and plays from its stored position if any

  Scenario: Pressing Enter opens the best match
    Given the strip shows at least one search-result card
    When the user presses Enter in the search box
    Then the first result card loads exactly as if clicked

  Scenario: Zero matches reports clearly
    Given the user typed a fragment matching nothing in the neighbour index
    When the strip repaints
    Then the strip keeps only the Open Video tile
    And a short inline hint states that nothing matched

  Scenario: Clearing restores the continue list
    Given the strip currently shows search results
    When the user empties the search box or presses Escape while it has focus
    Then the strip returns to the plain watch-later cards
```

## Notes
- Scope: catalog paths → each file’s parent dir + that dir’s sibling dirs (BFS queue of dirs, then non-recursive video listing per dir). Skip the filesystem root as a scan dir; do not list children of the root as sibling dirs. See [34](34-files-catalog.md) for the catalog; [07](07-sibling-folder-queue.md) for playback folder-advance (different feature).
- Seeds: `db::list_file_paths()` (table `files`). Discoveries from the session scan call `db::ensure_files` (one transaction) so later sessions grow the catalog.
- Reuses `video_ext::list_videos_in_dir` and natural lexical ordering (`lexical_sort`).
- Results capped (`SEARCH_MAX_HITS`); hint notes the cap. Hits may include continue-list members. Search-strip chrome omits Remove / Trash for every hit.
- Index: built once per window/session on continue-search bind (one idle) or on first committed query if still empty; typing never rescans. Filter debounce: `TYPE_DEBOUNCE_MS` in `src/recent_view/sibling_search_state.rs`; empty draft commits immediately.
- Placement: search row centered horizontally, parked just above the card strip; hint side slot mirrored by an invisible twin of the widest hint. macOS header-compositing band stays clear.
- Paint path: `RecentContext::refill` / `apply_strip`; draft vs committed query; skip identical neighbour paints; `fill_row` keeps Open Video.
- Escape while a text widget has focus proceeds to the search box (clear), not strip/playback shortcuts.
- Styling: `entry.search.rp-recent-search-entry` in `src/theme/continue_grid.css`.
