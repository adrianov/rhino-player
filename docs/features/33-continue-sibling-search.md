# Sibling search on the continue screen

---
status: done
priority: p1
layers: [ui, storage, persistence]
related: [07, 21, 34]
---

## Use cases
- Find a video the player already knows by typing a name fragment — exact pieces or close name similarity — including files already on the continue list.
- Surprise-browse a handful of playable catalogued titles without typing a query — a series shows the episode already in progress, or its first episode if none is.
- Draw another lucky handful without seeing the same titles again until every playable catalogued title has appeared once this session.
- Dismiss a lucky pick from the strip without moving the file to the platform trash.

## Description
The browse screen grows a **search box above the video card strip**, with an **I'm Feeling Lucky** control beside it. Search and I'm Feeling Lucky use only paths already in the media files catalog. They do not walk folders or discover videos that were never opened or otherwise registered. Typing matches catalogued file names case-insensitively: names that contain the query always qualify, and close name similarity can qualify the rest. Results appear as regular cards in the same horizontal strip, closer matches first; when names match equally, titles with a stored playback position appear before unstarted ones. **I'm Feeling Lucky** instead fills the strip with a small random handful of playable catalogued titles: a television series contributes one card — the episode that already has playback progress, or the first episode of that series if none does. Later lucky draws in the same session skip titles already shown until every playable catalogued title has appeared; then a new cycle may draw from the full list again. After each lucky handful the player prepares the next one off-screen and captures stills for it, so a later lucky click can show pictures that are already stored. Trashing or removing a lucky card puts another unused catalogued title in that slot when one remains. The strip switches between plain watch-later cards and those result cards in place — no navigation, no extra screen.

Result cards open like continue cards, including the hover resolution class tag after played percent. Present playable files show **Move to Trash** on hover like continue cards; **Remove from list** sits on the plain continue strip and on **I'm Feeling Lucky** cards (name-search hits omit it). Removing a lucky card leaves the file in place and keeps any stored playback position. Empty, hollow, or missing files are omitted from results. Missing stills are filled in the background like continue cards, and each card updates when its thumbnail is ready. Hovering a result updates the seek bar from stored length and resume like continue cards (no background load). While a query or I'm Feeling Lucky is active the strip shows only those result cards; clearing the box (or pressing Escape) restores the plain continue list. After the viewer closes a video, those cards show the latest stored progress, and a series lucky card is the episode now in progress. The strip and Open Video tile stay put while typing; cards update only after filtering settles.

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

  Scenario: Continue, search, and lucky cards share one strip display
    Given the continue screen can show watch-later cards, search hits, or I'm Feeling Lucky picks
    When the card strip paints for any of those populations
    Then each video card uses the same card layout as a watch-later card
    And the Open Video tile remains the first tile
    And hovering a present card updates the seek bar from stored length and resume like a watch-later card

  Scenario: Typing keeps the strip stable until filtering settles
    Given the continue screen shows the plain watch-later cards
    When the user types a search fragment without pausing long enough for filtering to finish
    Then the strip still shows the same cards it showed before typing
    And the Open Video tile does not disappear or rebuild

  Scenario: Settled typing swaps the strip to matching neighbour cards
    Given the media files catalog holds several video paths
    When the user types a fragment of one of those file names and filtering finishes
    Then the strip replaces the watch-later cards with one card per matching file
    And the Open Video tile remains the first tile without flashing away
    And each card carries that file's title and progress from the store when known

  Scenario: Search uses only catalogued paths
    Given the catalog holds one video
    And another video sits in the same folder but is not in the catalog
    When the user types a fragment of that other file's name and filtering finishes
    Then that other file does not appear among the result cards

  Scenario: Search and Lucky do not walk folders
    Given the catalog already holds several video paths
    When the user searches or activates I'm Feeling Lucky
    Then matching and picks use those catalogued paths
    And the player does not walk folders to discover more videos

  Scenario: The catalog index is reused for every query
    Given the player has already loaded catalogued paths for search this session
    When the user changes the search fragment and filtering finishes again
    Then matching uses those same catalogued paths without reading the catalog again

  Scenario: Matching ignores letter case
    Given the catalog holds video files with mixed-case names
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

  Scenario: Openability and thumbnails cover only strip cards
    Given the catalog holds many videos whose names match a short query
    When filtering finishes with a capped result strip
    Then openability checks run for videos placed on that strip
    And any further openability check only decides whether more playable matches exist past the strip limit
    And thumbnail work runs only for those same strip cards

  Scenario: Match hint claims a playable cap only when more playable hits exist
    Given more than the strip limit of catalogued names match the query
    And fewer than the strip limit of those matches are playable
    When filtering finishes
    Then the match hint counts only the playable hits on the strip
    And the match hint does not claim that more playable matches exist

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
    Given the media files catalog holds several playable videos
    When the user activates I'm Feeling Lucky
    Then the strip replaces the watch-later cards with a small handful of those playable videos
    And the Open Video tile remains the first tile
    And the shown titles need not match any typed fragment

  Scenario: Feeling Lucky skips unplayable neighbours
    Given the catalog holds playable files and hollow or missing files
    When the user activates I'm Feeling Lucky
    Then only playable neighbours appear among the result cards
    And the match hint counts only those playable picks

  Scenario: Feeling Lucky shows the in-progress episode of a series
    Given the catalog holds several episodes of one series
    And the viewer has playback progress on one of those episodes
    When the user activates I'm Feeling Lucky
    Then a result card for that series is the episode with progress
    And other episodes of that series do not appear as extra lucky cards

  Scenario: Feeling Lucky shows the first episode when a series is unstarted
    Given the catalog holds several episodes of one series
    And none of those episodes have playback progress
    When the user activates I'm Feeling Lucky
    Then a result card for that series is the first episode of that series
    And later episodes of that series do not appear as extra lucky cards

  Scenario: Feeling Lucky collapses episodes labeled with a number then an episode word
    Given the catalog holds several episodes of one series
    And those file names put an episode number before an episode word
    And none of those episodes have playback progress
    When the user activates I'm Feeling Lucky
    Then a result card for that series is the first episode of that series
    And later episodes of that series do not appear as extra lucky cards

  Scenario: Feeling Lucky collapses episodes that each sit in their own neighbouring folder
    Given the catalog holds several episodes of one series
    And each episode sits in its own folder beside the others
    And those folder names include the series title and an episode label
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
    And thumbnail work does not run for those prepared titles yet

  Scenario: Feeling Lucky thumbnails cover only on-screen picks
    Given I'm Feeling Lucky filled the strip
    When background thumbnail work runs
    Then stills are requested only for the lucky cards currently on the strip

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

  Scenario: Lucky cards show Remove and Trash
    Given I'm Feeling Lucky filled the strip with playable videos
    When the user rests the pointer on a lucky card
    Then the card shows Move to Trash
    And the card shows Remove from list

  Scenario: Remove on a lucky card fills the slot
    Given I'm Feeling Lucky filled the strip
    And more playable collected titles remain
    When the user activates Remove from list on a lucky card
    Then that file is not moved to the platform trash
    And the catalog still holds that path
    And another playable collected title occupies that card slot
    And the Open Video tile remains the first tile

  Scenario: Remove on a lucky card leaves a shorter strip when nothing remains
    Given I'm Feeling Lucky filled the strip
    And no other playable collected titles remain
    When the user activates Remove from list on a lucky card
    Then that file no longer appears on the strip
    And the remaining lucky cards stay
    And the file is not moved to the platform trash

  Scenario: Remove on a lucky card keeps stored progress
    Given I'm Feeling Lucky showed a title that already has a stored playback position
    When the user activates Remove from list on that lucky card
    Then the persistent store still holds that playback position

  Scenario: Feeling Lucky with nothing playable stays on Open Video
    Given the catalog has no playable videos
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
- Scope: search and I'm Feeling Lucky use `CatalogMem` (`sibling_search.rs`): one in-memory load of `db::list_file_paths` plus resume/duration maps per window; name hits, Lucky roll/retarget/refill, and paint keys go through that API only (no SQLite while filtering). Progress maps refresh on Lucky roll and when browse becomes visible again (after playback). No folder walk, no `list_videos_in_dir`, no `ensure_files`. See [34](34-files-catalog.md) for the catalog; [07](07-sibling-folder-queue.md) for playback folder-advance (different feature).
- Index: `CatalogMem` holds catalog paths plus lowercased names; built once per window on continue-search bind (one idle) or on first committed query if still empty. Typing never rereads the catalog and never clones the index to filter. Hit order: name score descending; equal scores prefer a non-zero stored resume from the cached maps by listing path or file name (`progress_name_keys` — rebuilt only when progress reloads); then natural lexical name (`lexical_sort`). Wide queries collect name matches then fill the strip in ranked batches (`RANK_BATCH`) so unopenable prefixes cannot starve lower openable hits. Filter debounce: `TYPE_DEBOUNCE_MS` in `src/recent_view/sibling_search_state.rs`; empty draft commits immediately.
- Scoring: `name_match_score` in `sibling_search_score.rs`. A name that contains the query ranks with `substring_score` (token/name prefix first; one-letter skips the token walk) and never builds trigram sets. Trigram Jaccard (`TRIGRAM_MIN_SCORE`, best of the full name and each alphanumeric token) runs only for queries of three or more characters that are not a substring, so a misspelled word still hits. Results capped (`SEARCH_MAX_HITS` via `capped_name_hits`); the hint’s `{n}+` form is set only when another **playable** hit remains past the strip (`has_openable_left` on the leftover ranked pool — stops at the first yes). Hits may include continue-list members. Name ranking is in memory; hollow/missing preflight (`media_open_fail::preflight_user_message` via `NeighbourEntry::is_openable`) runs while filling strip slots from each ranked batch, then only as far as needed for that overflow flag — thumbnails still cover painted cards only. Lucky title grouping (`openable_set`) skips known-unopenable paths without forcing preflight on the whole catalog; `keep_openable` preflights only paths about to appear on the strip (shown handful, or a reserved path promoted into a slot). Trash/restore go through `recent_view::card_trashed` / `note_path_trashed` / `search_note_restored` (catalog forget + strip context owns the index). Search-strip chrome shows Move to Trash for present files; omits Remove (list membership) for name-search hits (`StripKind::NeighbourHits`). Lucky cards use `StripKind::Lucky` (Trash + Remove); Remove calls `card_removed` (`fill_lucky_gap` without `forget_file` or an openability drop) so the file and any resume stay.
- Placement: search row centered horizontally, parked just above the card strip; **I'm Feeling Lucky** sits to the right of the entry (`lucky_button` in `sibling_search_widgets.rs`) with an invisible twin on the left so the field stays centered; match hint sits under the field. Undo / notice pills overlay the bottom spacer ([21](21-recent-videos-launch.md)) so they do not shift this row. macOS header-compositing band stays clear. Placeholder: `Search your video library…`. First map clears initial focus from the entry (GTK would otherwise focus the first focusable field). Playback uses `dismiss_search_for_playback` before the continue strip hides — including at the start of a warm-reveal beat — then `hide_continue_strip` unmaps the strip. Dismiss drops window focus, sets the inner text `im-module` to `gtk-im-context-none` (tears down IBus / gdk-macos IM so the status mark cannot orphan over video), then unmaps the row; an idle repeats the IM drop after unmap. Strip `notify::visible` restores the default IM module and remaps the row when browse returns.
- **I'm Feeling Lucky:** `LuckySession` (`src/recent_view/lucky.rs`) groups session-index paths into titles once (`titles::group_index`, cached for the window) and filters cached `NeighbourEntry` openability when picking. Episode-like names (`SxxExx` / `Sxx.Exx` / `NxNN` / Episode / `серия` / `сер.` with the number before or after, including `(11 сер.)`) share one title; season-named parent folders use `folder_series_stem` / `folder_looks_seasonal` (`sibling_advance` series helpers, same stem rules as [07](07-sibling-folder-queue.md)); a parent folder whose own name carries an episode marker groups sibling episode folders under the enclosing directory. Each series title picks the in-progress episode from the cached progress maps by listing path or file name (same store keys as search `progress_name_keys` — no canonicalize / entity resolve), skipping `past_done_mark`, or else the first path in natural lexical order; standalones stay one file each. Returning to browse retargets only the shown and reserved handfuls (`LuckySession::retarget` / `titles::retarget_lists`) so a series card follows the watching episode. One shuffle of the title list yields the shown handful and the reserved next (`take_ready_then_next`), each capped at `CONTINUE_DISPLAY_MAX`. `LuckySession` skips titles already drawn until the unused pool is empty, then clears and draws from the full list again. After each shown handful it reserves the following draw (same pick rules) for the next click; `ThumbBackfill::schedule` covers only the painted strip paths (no off-screen warm thumbs for the reserved next). A later lucky click consumes the reserved handful (then reserves again) instead of rolling a new sample. Trash or Remove on a shown lucky card calls `fill_lucky_gap` (`lucky/gap.rs`): the gone path leaves the snapshot, a replacement is taken from `lucky_next` when possible or `take_one_title` (unseen first, never an on-screen duplicate), inserted in the same slot, and `lucky_next` is topped up. Card actions keep the listing path (`card_data_list`) so `card_trashed` / `card_removed` match the lucky snapshot after canonicalize. Trash also forgets the catalog path; Remove does not. Search hits still omit the trashed path only. Paint-time `keep_openable` drops snapshot paths marked unopenable so Undo restore can show a search card again. Enter opens the first pick without committing an empty query (that would dismiss lucky). Clearing the box / Escape drops lucky mode and restores watch-later (session seen stays); a typed query replaces lucky cards with name hits. Hint strings live next to the sample (`lucky_hint` / `search_hint`). Styling: `button.rp-recent-lucky` in `src/theme/continue_grid.css`.
- Paint path: one entry `RecentContext::apply_strip` / `ensure_apply_strip` for continue list, search hits, and I'm Feeling Lucky (boot included — no separate `fill_continue_strip` paint). `strip_plan` picks paths + `StripKind`; `fill_row` / `append_history_card` build every video card the same way. `StripKind` only gates Remove visibility (`shows_remove`) and neighbour paint-skip (`hits_strip`); seek-bar chrome cache refreshes for every kind so hover sync matches watch-later. Thumbs: `ThumbBackfill::schedule` for the painted paths only; ready stills hop via coalesced `MainContext::invoke` in `live_card` → in-place `apply_ready_thumbs`. Draft vs committed query; skip neighbour paints only when paths **and** stored resume/duration match (`sibling_search_paint.rs`); `fill_row` keeps Open Video. Lucky / search neighbours are usually never-watched so they miss the SQLite still cache; the same backfill as continue cards fills them, with parallel workers so the handful is not one-file-at-a-time. A still that cannot be parsed drops the path (`forget_file` + index drop) and refills a lucky slot like trash.
- Escape while a text widget has focus proceeds to the search box (clear), not strip/playback shortcuts.
- Styling: `entry.search.rp-recent-search-entry` in `src/theme/continue_grid.css`. No-thumb placeholder uses bundled `camera-video-symbolic`.
