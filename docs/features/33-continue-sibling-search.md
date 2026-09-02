# Sibling search on the continue screen

---
status: done
priority: p1
layers: [ui, storage, persistence]
related: [07, 21, 34]
---

## Use cases
- Find the next episode sitting next to something already known to the player without going back through **Open Video**.
- Jump straight to any neighbouring video whose file name matches a typed fragment — exact pieces or close name similarity — including files already on the continue list.
- Reach videos in a folder next to a known title’s folder (same parent), without scanning the whole disk or other top-level libraries.
- Surprise-browse a handful of playable titles from that collected neighbour list without typing a query.

## Description
The browse screen grows a **search box above the video card strip**, with an **I'm Feeling Lucky** control beside it. Once per session the player gathers searchable neighbours from every path in the media files catalog: video files in each known file’s folder and in that folder’s sibling folders (folders that share the same parent). It never lists the filesystem root and never treats top-level folders as siblings of each other (for example it will not walk from one library root to another under `/`). Typing matches neighbour file names case-insensitively: names that contain the query always qualify, and close name similarity can qualify the rest. Results appear as regular cards in the same horizontal strip, closer matches first; when names match equally, titles with a stored playback position appear before unstarted ones. **I'm Feeling Lucky** instead fills the strip with a small random handful of playable collected neighbours. The strip switches between plain watch-later cards and those result cards in place — no navigation, no extra screen.

Result cards open like continue cards. Present playable files show **Move to Trash** on hover like continue cards; **Remove from list** stays on the plain continue strip only. Empty, hollow, or missing files are omitted from results. Missing stills are filled in the background like continue cards, and each card updates when its thumbnail is ready. Hovering a result updates the seek bar from stored length and resume like continue cards (no background load). While a query or I'm Feeling Lucky is active the strip shows only those result cards; clearing the box (or pressing Escape) restores the plain continue list. The strip and Open Video tile stay put while typing; cards update only after filtering settles.

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
    And an I'm Feeling Lucky control sits beside the search box
    And the search box is not focused
    And the search box placeholder invites searching the video library
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

  Scenario: Neighbour folders are not rescanned for every query
    Given the player has already gathered searchable neighbours this session
    When the user changes the search fragment and filtering finishes again
    Then matching uses the neighbours already gathered without walking those folders again

  Scenario: Matching ignores letter case
    Given neighbour folders contain video files with mixed-case names
    When the user types a fragment differing only in letter case
    Then every neighbour whose file name contains that fragment ignoring case appears
    And neighbours that neither contain the fragment nor resemble it closely enough do not appear

  Scenario: Closer name matches rank ahead of weaker ones
    Given two playable neighbours both match the same query
    And one file name resembles the query more closely than the other
    When filtering finishes
    Then the closer match appears before the weaker match among the result cards

  Scenario: In-progress videos rank ahead when name match is equal
    Given two playable neighbours both match the same query equally closely
    And one has a non-zero stored playback position and the other does not
    When filtering finishes
    Then the in-progress neighbour appears before the unstarted one among the result cards

  Scenario: Slightly misspelled fragments still match
    Given a playable neighbour whose file name contains a recognizable word
    When the user types that word with one or two letter mistakes and filtering finishes
    Then that neighbour appears among the result cards

  Scenario: Continue-list files remain searchable
    Given a matching file is already on the watch-later list
    When the user searches with a fragment that matches it
    Then that file appears among the result cards

  Scenario: Empty or hollow neighbours stay out of results
    Given a neighbour path matches the query by name
    And that path's bytes are missing or all zeroes so it cannot be opened
    When filtering finishes
    Then that path does not appear among the result cards
    And the match hint counts only playable hits

  Scenario: Search results get background thumbnails
    Given the strip shows search-result cards for present local files that have no stored still
    When background thumbnail work finishes for those files
    Then each of those cards shows its thumbnail without clearing the search
    And the Open Video tile stays in place

  Scenario: Result cards open with trash and seek-bar hover sync
    Given the strip shows search-result cards for present local files
    When the user rests the pointer on a result card
    Then the seek bar shows that card’s stored length and resume position like a hovered continue card
    And the card shows Move to Trash
    And the card shows no Remove from list control
    When the user clicks the result card
    Then that file loads and plays from its stored position if any

  Scenario: Trash on a search result removes the file
    Given the strip shows a search-result card for a present local file
    When the user activates Move to Trash on that card
    Then the file is moved to the platform trash
    And that file no longer appears among later search results for the same query
    And the player does not re-check the trashed path on disk to omit it

  Scenario: Pressing Enter opens the best match
    Given the strip shows at least one search-result card
    When the user presses Enter in the search box
    Then the first result card loads exactly as if clicked

  Scenario: Pressing Enter after Feeling Lucky opens the first pick
    Given I'm Feeling Lucky filled the strip with at least one playable video
    When the user presses Enter in the search box
    Then the first result card loads exactly as if clicked
    And the lucky handful is not cleared first

  Scenario: Zero matches reports clearly
    Given the user typed a fragment matching no searchable neighbour
    When the strip repaints
    Then the strip keeps only the Open Video tile
    And a short inline hint states that nothing matched

  Scenario: Opening playback hides the search box
    Given the search box has focus
    When the user opens a video and the continue strip hides
    Then the search box is not visible
    And the I'm Feeling Lucky control is not visible
    And no text caret or typed character from the search box appears over the video

  Scenario: Feeling Lucky fills the strip from the collected library
    Given the media files catalog has gathered several playable neighbour videos
    When the user activates I'm Feeling Lucky
    Then the strip replaces the watch-later cards with a small handful of those playable videos
    And the Open Video tile remains the first tile
    And the shown titles need not match any typed fragment

  Scenario: Feeling Lucky skips unplayable neighbours
    Given the collected neighbour list includes playable files and hollow or missing files
    When the user activates I'm Feeling Lucky
    Then only playable neighbours appear among the result cards
    And the match hint counts only those playable picks

  Scenario: Feeling Lucky again offers another handful
    Given I'm Feeling Lucky already filled the strip
    And the collected playable list is larger than one handful
    When the user activates I'm Feeling Lucky again
    Then the strip shows another handful drawn from that list

  Scenario: Escape after Feeling Lucky restores the continue list
    Given I'm Feeling Lucky filled the strip
    When the user clears the search box or presses Escape while typing
    Then the strip shows the Open Video tile followed by the usual watch-later cards

  Scenario: Typing after Feeling Lucky searches as usual
    Given I'm Feeling Lucky filled the strip
    When the user types a fragment of a neighbour file name and filtering finishes
    Then the strip replaces the lucky picks with one card per matching file

  Scenario: Feeling Lucky with nothing playable stays on Open Video
    Given the collected neighbour list has no playable videos
    When the user activates I'm Feeling Lucky
    Then the strip keeps only the Open Video tile
    And a short inline hint states that nothing could be picked
```

## Notes
- Scope: catalog paths → each file’s parent dir + that dir’s sibling dirs (BFS queue of dirs, then non-recursive video listing per dir). Skip the filesystem root as a scan dir; do not list children of the root as sibling dirs. See [34](34-files-catalog.md) for the catalog; [07](07-sibling-folder-queue.md) for playback folder-advance (different feature).
- Seeds: `db::list_file_paths()` (table `files`). Discoveries from the session scan call `db::ensure_files` (one transaction) so later sessions grow the catalog.
- Reuses `video_ext::list_videos_in_dir`. Hit order: trigram Jaccard score descending; equal scores prefer a non-zero resume from the same maps as card progress (`card_resume_duration` / `load_time_pos_map` + `load_duration_map`); then natural lexical name (`lexical_sort`).
- Scoring: padded character trigrams + Jaccard (`sibling_search_score.rs`). Score is the best Jaccard of the query against the full lowercased file name and each alphanumeric token (so a misspelled word inside a long name still ranks without sliding-window noise); minimum `TRIGRAM_MIN_SCORE`. Substring containment always keeps a hit even when Jaccard is low. Results capped (`SEARCH_MAX_HITS`); hint notes the cap. Hits may include continue-list members. Openability is classified once when the session neighbour index is built (`NeighbourEntry.openable` via `media_open_fail::preflight_user_message`); settled queries filter that flag only. Trash/restore go through `recent_view::search_note_removed` / `search_note_restored` (strip context owns the index). Search-strip chrome shows Move to Trash for present files; omits Remove (list membership) for every hit.
- Index: built once per window/session on continue-search bind (one idle) or on first committed query if still empty; typing never rescans. Filter debounce: `TYPE_DEBOUNCE_MS` in `src/recent_view/sibling_search_state.rs`; empty draft commits immediately.
- Placement: search row centered horizontally, parked just above the card strip; **I'm Feeling Lucky** sits to the right of the entry (`lucky_button` in `sibling_search_widgets.rs`) with an invisible twin on the left so the field stays centered; match hint sits under the field. macOS header-compositing band stays clear. Placeholder: `Search your video library…`. First map clears initial focus from the entry (GTK would otherwise focus the first focusable field). Playback uses `dismiss_search_for_playback` (focus drop + search-row unmap) before the continue strip hides — including at the start of a warm-reveal beat — then `hide_continue_strip` unmaps the strip. Strip `notify::visible` restores the row when browse returns.
- **I'm Feeling Lucky:** `lucky_picks` (`sibling_search_lucky.rs`) samples openable session-index paths (same `NeighbourEntry.openable` gate as search), shuffles, then caps at `CONTINUE_DISPLAY_MAX` so the handful matches the usual continue-strip card count. Another click reshuffles. Paint-time `keep_openable` drops snapshot paths marked unopenable (trash) so Undo restore shows the card again. Enter opens the first pick without committing an empty query (that would dismiss lucky). Clearing the box / Escape drops lucky mode and restores watch-later; a typed query replaces lucky cards with name hits. Hint strings live next to the sample (`lucky_hint` / `search_hint`). Styling: `button.rp-recent-lucky` in `src/theme/continue_grid.css`.
- Paint path: `RecentContext::apply_strip` / `ensure_apply_strip` (arms `ThumbBackfill::schedule`); ready stills hop via coalesced `MainContext::invoke` in `live_card` (no refill poll) → in-place `apply_ready_thumbs`; draft vs committed query; skip identical neighbour paints; `fill_row` keeps Open Video.
- Escape while a text widget has focus proceeds to the search box (clear), not strip/playback shortcuts.
- Styling: `entry.search.rp-recent-search-entry` in `src/theme/continue_grid.css`. No-thumb placeholder uses bundled `camera-video-symbolic`.
