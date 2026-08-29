# Sibling search on the continue screen

---
status: done
priority: p1
layers: [ui, storage]
related: [07, 21]
---

## Use cases
- Find the next episode sitting next to something on the watch-later list without going back through **Open Video**.
- Jump straight to any neighbouring video whose file name contains a known fragment (episode number, title word, release tag), including files already on the continue list.

## Description
The browse screen grows a **search box above the video card strip**. Typing scans the directories that hold watch-later entries for video files and shows every file whose name contains the typed fragment (letter case ignored) as a regular card in the same horizontal strip — including files that are already on the continue list. The strip switches between plain watch-later cards and search-result cards in place — no navigation, no extra screen.

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
    Given a watch-later entry references a file inside a folder that also holds other video files
    When the user types a fragment of one of those neighbour file names and filtering finishes
    Then the strip replaces the watch-later cards with one card per matching file
    And the Open Video tile remains the first tile without flashing away
    And each card carries that file's title and progress from the store when known

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
    Given the user typed a fragment matching nothing near the watch-later entries
    When the strip repaints
    Then the strip keeps only the Open Video tile
    And a short inline hint states that nothing matched

  Scenario: Clearing restores the continue list
    Given the strip currently shows search results
    When the user empties the search box or presses Escape while it has focus
    Then the strip returns to the plain watch-later cards
```

## Notes
- Scope: **direct siblings only** — the immediate parent directories of watch-later entries (full list, not just the five shown), listed non-recursively. Sub-directory trees beside them are not walked (see [07](07-sibling-folder-queue.md) for folder-advance semantics).
- Reuses the shared extension list (`video_ext`) and natural lexical ordering (`lexical_sort`) from open / folder scan.
- Results are capped (`SEARCH_MAX_HITS`); the hint notes the cap. Hits may include continue-list members; `card_data_list` still supplies store progress/thumbs when present. Search-strip chrome omits Remove / Trash for every hit.
- Directory listings rebuild lazily and throttle-rescan on typing activity (no timers otherwise); this follows the synchronous `read_dir` precedent of sibling advance. StaleListing risk on network mounts equals existing folder-scan behaviour.
- Placement: the search row is **centered horizontally** and **parked just above the card strip** (top expand spacer, margin under the row). Match hint uses a fixed-width side slot mirrored by an invisible twin of the widest hint (`40+ matches`) so the entry stays centered when the hint has text. Stays clear of the macOS header-compositing band at the very top of the overlay.
- Query awareness lives in the paint path (`RecentContext::refill`, `repaint_continue_row`) so background thumbnail refills cannot clobber active results. State type: `SiblingSearchState` in `src/recent_view/sibling_search.rs`.
- The entry text is **draft** until typing debounce commits it (`TYPE_DEBOUNCE_MS`); only the committed query drives `current_hits` / strip paint. Thumb-driven `refill` and other paints are skipped while a draft is pending; settled neighbour paints with identical paths are skipped in `SiblingSearchState`. Search commits call `apply_strip`. `fill_row` keeps the Open Video tile and only replaces trailing cards.
- Escape precedence: while a text widget owns focus, the capture-phase shortcut pass lets Escape proceed (`KeyDispatch::dispatch` guard), so the search box consumes it (clear) instead of triggering playback shortcuts or strip escapes.
- Styling: one surface on `entry.search.rp-recent-search-entry` in `src/theme/continue_grid.css` (GtkSearchEntry’s CSS node is `entry.search`; no outer wrapper). Base rules shared; `macos_native_lists.css` paddings untouched.
