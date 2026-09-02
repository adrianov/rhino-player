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
- Surprise-browse a handful of playable titles from that collected neighbour list without typing a query — a series shows the episode already in progress, or its first episode if none is.
- Draw another lucky handful without seeing the same titles again until every playable collected title has appeared once this session.

## Description
The browse screen grows a **search box above the video card strip**, with an **I'm Feeling Lucky** control beside it. Once per session the player gathers searchable neighbours from every path in the media files catalog: video files in each known file’s folder and in that folder’s sibling folders (folders that share the same parent). It never lists the filesystem root and never treats top-level folders as siblings of each other (for example it will not walk from one library root to another under `/`). Typing matches neighbour file names case-insensitively: names that contain the query always qualify, and close name similarity can qualify the rest. Results appear as regular cards in the same horizontal strip, closer matches first; when names match equally, titles with a stored playback position appear before unstarted ones. **I'm Feeling Lucky** instead fills the strip with a small random handful of playable collected neighbours: a television series contributes one card — the episode that already has playback progress, or the first episode of that series if none does. Later lucky draws in the same session skip titles already shown until every playable collected title has appeared; then a new cycle may draw from the full list again. After each lucky handful the player prepares the next one off-screen and captures stills for it, so a later lucky click can show pictures that are already stored. Trashing a lucky card puts another unused collected title in that slot when one remains. The strip switches between plain watch-later cards and those result cards in place — no navigation, no extra screen.

Result cards open like continue cards, including the hover resolution class tag after played percent. Present playable files show **Move to Trash** on hover like continue cards; **Remove from list** stays on the plain continue strip only. Empty, hollow, or missing files are omitted from results. Missing stills are filled in the background like continue cards, and each card updates when its thumbnail is ready. Hovering a result updates the seek bar from stored length and resume like continue cards (no background load). While a query or I'm Feeling Lucky is active the strip shows only those result cards; clearing the box (or pressing Escape) restores the plain continue list. After the viewer closes a video, those cards show the latest stored progress, and a series lucky card is the episode now in progress. The strip and Open Video tile stay put while typing; cards update only after filtering settles.

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

  Scenario: Unparseable neighbours leave results
    Given the strip shows a result card for a present local file
    When background thumbnail work cannot parse that file
    Then that file no longer appears among later search or lucky cards

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
    And the catalog no longer holds that path
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
    And no text caret, typed character, or input-method mark from the search box appears over the video

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

  Scenario: Feeling Lucky shows the in-progress episode of a series
    Given the collected neighbour list includes several episodes of one series
    And the viewer has playback progress on one of those episodes
    When the user activates I'm Feeling Lucky
    Then a result card for that series is the episode with progress
    And other episodes of that series do not appear as extra lucky cards

  Scenario: Feeling Lucky shows the first episode when a series is unstarted
    Given the collected neighbour list includes several episodes of one series
    And none of those episodes have playback progress
    When the user activates I'm Feeling Lucky
    Then a result card for that series is the first episode of that series
    And later episodes of that series do not appear as extra lucky cards

  Scenario: Feeling Lucky does not repeat titles until the list is exhausted
    Given the collected playable list is larger than one handful
    And I'm Feeling Lucky already filled the strip
    When the user activates I'm Feeling Lucky again
    Then the new handful has no title that appeared in an earlier handful this session

  Scenario: Feeling Lucky may reuse titles after the list is exhausted
    Given every playable collected title has already appeared in an I'm Feeling Lucky handful this session
    When the user activates I'm Feeling Lucky again
    Then the strip shows a new handful drawn from the full playable list

  Scenario: Feeling Lucky prepares the next handful off-screen
    Given I'm Feeling Lucky already filled the strip
    And more playable collected titles remain
    When the player prepares the next lucky handful
    Then the strip still shows the current handful
    And stills for the prepared titles are stored without changing those cards

  Scenario: Feeling Lucky uses the prepared handful
    Given a next lucky handful was prepared
    When the user activates I'm Feeling Lucky again
    Then the strip shows that prepared handful

  Scenario: Escape after Feeling Lucky restores the continue list
    Given I'm Feeling Lucky filled the strip
    When the user clears the search box or presses Escape while typing
    Then the strip shows the Open Video tile followed by the usual watch-later cards

  Scenario: Typing after Feeling Lucky searches as usual
    Given I'm Feeling Lucky filled the strip
    When the user types a fragment of a neighbour file name and filtering finishes
    Then the strip replaces the lucky picks with one card per matching file

  Scenario: Trash on a lucky card fills the slot
    Given I'm Feeling Lucky filled the strip
    And more playable collected titles remain
    When the user activates Move to Trash on a lucky card
    Then that file is moved to the platform trash
    And the catalog no longer holds that path
    And another playable collected title occupies that card slot
    And the Open Video tile remains the first tile

  Scenario: Trash on a lucky card leaves a shorter strip when nothing remains
    Given I'm Feeling Lucky filled the strip
    And no other playable collected titles remain
    When the user activates Move to Trash on a lucky card
    Then that file no longer appears on the strip
    And the remaining lucky cards stay

  Scenario: Feeling Lucky with nothing playable stays on Open Video
    Given the collected neighbour list has no playable videos
    When the user activates I'm Feeling Lucky
    Then the strip keeps only the Open Video tile
    And a short inline hint states that nothing could be picked

  Scenario: Result cards show stored progress after playback
    Given I'm Feeling Lucky or a settled search filled the strip
    And the viewer opened a result card and played into that title
    When the viewer closes the video and the continue screen returns
    Then that card shows the stored playback progress

  Scenario: Feeling Lucky series card follows the watching episode after playback
    Given I'm Feeling Lucky showed one episode of a series
    And the viewer now has playback progress on a different episode of that series
    When the continue screen returns
    Then that series card is the episode with progress
```

## Notes
- Scope: catalog paths → each file’s parent dir + that dir’s sibling dirs (BFS queue of dirs, then non-recursive video listing per dir). Skip the filesystem root as a scan dir; do not list children of the root as sibling dirs. See [34](34-files-catalog.md) for the catalog; [07](07-sibling-folder-queue.md) for playback folder-advance (different feature).
- Seeds: `db::list_file_paths()` (table `files`). Discoveries from the session scan call `db::ensure_files` (one transaction) so later sessions grow the catalog.
- Reuses `video_ext::list_videos_in_dir` (`.ts` only when `ts_file_is_video`, see [34](34-files-catalog.md)). Hit order: trigram Jaccard score descending; equal scores prefer a non-zero resume from the same maps as card progress (`card_resume_duration` / `load_time_pos_map` + `load_duration_map`); then natural lexical name (`lexical_sort`).
- Scoring: padded character trigrams + Jaccard (`sibling_search_score.rs`). Score is the best Jaccard of the query against the full lowercased file name and each alphanumeric token (so a misspelled word inside a long name still ranks without sliding-window noise); minimum `TRIGRAM_MIN_SCORE`. Substring containment always keeps a hit even when Jaccard is low. Results capped (`SEARCH_MAX_HITS`); hint notes the cap. Hits may include continue-list members. Openability is classified once when the session neighbour index is built (`NeighbourEntry.openable` via `media_open_fail::preflight_user_message`); settled queries filter that flag only. Trash/restore go through `recent_view::note_path_trashed` / `search_note_restored` (catalog forget + strip context owns the index). Search-strip chrome shows Move to Trash for present files; omits Remove (list membership) for every hit.
- Index: built once per window/session on continue-search bind (one idle) or on first committed query if still empty; typing never rescans. Filter debounce: `TYPE_DEBOUNCE_MS` in `src/recent_view/sibling_search_state.rs`; empty draft commits immediately.
- Placement: search row centered horizontally, parked just above the card strip; **I'm Feeling Lucky** sits to the right of the entry (`lucky_button` in `sibling_search_widgets.rs`) with an invisible twin on the left so the field stays centered; match hint sits under the field. macOS header-compositing band stays clear. Placeholder: `Search your video library…`. First map clears initial focus from the entry (GTK would otherwise focus the first focusable field). Playback uses `dismiss_search_for_playback` before the continue strip hides — including at the start of a warm-reveal beat — then `hide_continue_strip` unmaps the strip. Dismiss drops window focus, sets the inner text `im-module` to `gtk-im-context-none` (tears down IBus / gdk-macos IM so the status mark cannot orphan over video), then unmaps the row; an idle repeats the IM drop after unmap. Strip `notify::visible` restores the default IM module and remaps the row when browse returns.
- **I'm Feeling Lucky:** `LuckySession` (`src/recent_view/lucky.rs`) groups openable session-index paths into titles (same `NeighbourEntry.openable` gate as search). Episode-like names (`SxxExx` / `NxNN` / Episode) and season-named parent folders share one title via `folder_series_stem` / `folder_looks_seasonal` (`sibling_advance` series helpers, same stem rules as [07](07-sibling-folder-queue.md)). Each series title picks the in-progress episode (`card_resume_duration`, not past `past_done_mark`) or else the first path in natural lexical order; standalones stay one file each. Returning to browse retargets the shown and reserved handfuls through the same pick (`LuckySession::retarget` / `titles::retarget_paths`) so a series card follows the watching episode. The title list is shuffled and capped at `CONTINUE_DISPLAY_MAX`. `LuckySession` skips titles already drawn until the unused pool is empty, then clears and draws from the full list again. After each shown handful it reserves the following draw (same pick rules) and `RecentContext::schedule_thumbs` appends those paths to `ThumbBackfill::schedule` after the visible strip so stills land in the store before the next click; ready-path flush is a no-op for cards not on screen. A later lucky click consumes the reserved handful (then reserves again) instead of rolling a new sample. Trash on a shown lucky card calls `fill_lucky_gap` (`lucky/gap.rs`): the gone path leaves the snapshot, a replacement is taken from `lucky_next` when possible (already warming) or `take_one_title` (unseen first, never an on-screen duplicate), inserted in the same slot, and `lucky_next` is topped up. Search hits still omit the trashed path only. Paint-time `keep_openable` drops snapshot paths marked unopenable so Undo restore can show a search card again. Enter opens the first pick without committing an empty query (that would dismiss lucky). Clearing the box / Escape drops lucky mode and restores watch-later (session seen stays); a typed query replaces lucky cards with name hits. Hint strings live next to the sample (`lucky_hint` / `search_hint`). Styling: `button.rp-recent-lucky` in `src/theme/continue_grid.css`.
- Paint path: `RecentContext::apply_strip` / `ensure_apply_strip` (arms `ThumbBackfill::schedule`); ready stills hop via coalesced `MainContext::invoke` in `live_card` (no refill poll) → in-place `apply_ready_thumbs`; draft vs committed query; skip neighbour paints only when paths **and** stored resume/duration match (`sibling_search_paint.rs`); `fill_row` keeps Open Video. Lucky / search neighbours are usually never-watched so they miss the SQLite still cache; the same backfill as continue cards fills them, with parallel workers so the handful is not one-file-at-a-time. A still that cannot be parsed drops the path (`forget_file` + index drop) and refills a lucky slot like trash.
- Escape while a text widget has focus proceeds to the search box (clear), not strip/playback shortcuts.
- Styling: `entry.search.rp-recent-search-entry` in `src/theme/continue_grid.css`. No-thumb placeholder uses bundled `camera-video-symbolic`.
