# Recent videos grid on empty launch

---
status: done
priority: p1
layers: [ui, db, mpv, fs]
related: [03, 06, 13, 14, 17, 18, 23, 27]
settings: [seek_bar_preview]
mpv_props: [path, time-pos, duration, eof-reached]
---

## Use cases
- Launch from the icon and resume what you were watching with one click.
- See up to five recent files at a glance, with a thumbnail and progress.
- Drop entries from the list (with undo) when you no longer want to resume them.

## Description
On empty launch (no CLI paths, no other "open this first" path takes over the first paint), the main content shows a row that always begins with **Open Video** (same workflow as choosing a file from the main menu). Up to **five** continue cards follow in most-recently-opened order when history entries exist; when history is empty, only this tile appears. Each history card has a thumbnail (cover style), the filename (no ellipsis), a thin progress bar with numeric percent, and trash + remove controls on hover.

Clicking a history card loads that file and unpauses, even if watch-later had stored a paused session. The first history card may be warm-preloaded paused behind the grid; activating it (click or Space) hides the grid and reveals playback after a short reveal delay. Returning to the grid keeps the current file paused for warm reuse when the continue strip stays visible (including empty history while using this launch pattern). Playback stops when browsing back hides the strip (no boot-file launch paths).

History is durable, deduplicated by canonical path, capped at 20 entries (showing five), and prunes missing files on `history::load`. Thumbnails are JPEG BLOBs in the SQLite `media` table, refreshed in the background by a `vo=image` libmpv decode near the stored continue position. Remove and Move-to-trash share a session **LIFO undo stack** with a 10 s snackbar.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui @area:recent
Feature: Recent videos grid on empty launch

  Background:
    Given the SQLite history and media tables exist under ~/.config/rhino/rhino.sqlite

  Scenario: Continue strip appears on empty launch with valid history
    Given the first window is shown with no CLI paths and no session takeover
    And history contains at least one valid local entry
    When the window paints
    Then an Open Video tile is visible ahead of recent entries
    And up to five history cards appear after it most-recent-first
    And each history card shows a thumbnail, filename, and percent progress

  Scenario: Empty history still shows the continue strip with Open Video
    Given the first window is shown with no CLI paths and no session takeover
    And history is empty
    When the window paints
    Then the continue strip is visible with one primary tile
    And activating that tile chooses a media file through the same flow as opening from the main menu
    And no thumbnails from history appear beside it

  Scenario: Clicking a card opens and unpauses
    Given a continue card is visible and references a local file
    When the user activates the card
    Then loadfile completes for that path
    And mpv pause becomes false
    And the grid hides after the warm-reveal delay

  Scenario: Warm preload reveal on Space
    Given the recent grid is visible and the first card is warm-preloaded paused
    When the user presses Space
    Then after WARM_REVEAL_DELAY_MS the grid hides, chrome reveals, the window presents, and pause clears

  Scenario: Card layout uses full filename and percent
    Given a card is rendered for an existing file
    When the user reads the card
    Then the title shows the last path segment without ellipsis (word-wrapped)
    And the progress bar shows numeric percent (0% if never started, 100% when finished)

  Scenario: Remove from list with undo
    Given a card shows a remove control on hover
    When the user activates remove
    Then the entry is removed from history without confirmation
    And the watch-later sidecar and SQLite resume for that path are cleared
    And a snackbar offers Undo for 10 seconds
    And dismissing the snackbar discards the undo for that entry

  Scenario: Move to trash with undo
    Given a card represents an existing local file
    When the user activates trash
    Then the file is moved to the Freedesktop trash
    And history and resume are cleared for that path
    And the snackbar offers Undo when the trashed files/… copy is locatable
    And Undo restores the file plus the captured watch-later and media snapshot

  Scenario: Completed file leaves continue list on switch
    Given a local file reaches natural end or near-end criteria while another file loads
    When sibling advance or user switch fires
    Then the completed path is removed from history
    And resume is cleared for that path

  Scenario: Padding double-click toggles fullscreen
    Given the grid is visible with spacer padding around the card row
    When the user double-clicks primary on the spacers (not on a card or the undo bar)
    Then fullscreen toggles like the main video surface

  Scenario: Stale card shows greyed art and click removes
    Given a history entry exists for a path that no longer resolves on disk between refreshes
    When the user clicks the stale card
    Then the entry is removed from history via the on_stale path

  Scenario: Thumbnails refresh near stored continue position
    Given a card lacks a thumb or its stored thumb_time_pos differs from the current continue position by more than the freshness window
    When the background backfill runs
    Then a one-shot vo=image libmpv generates a JPEG near the stored time_pos
    And the new BLOB replaces the previous one in the media table
```

## Notes
- Trigger: empty CLI args; first paint follows this grid and CLI rules in [06-open-and-cli](06-open-and-cli.md).
- Deduplication: opening a path moves it to the front; capacity 20, display 5; `history::load` prunes missing files.
- Card UI: each card uses about 40% of the strip width with a minimum size, with a fixed **16:9** thumbnail frame (width drives height); image uses cover style (no letterboxing); title and progress sit in a soft bottom gradient overlay; the percentage is a small translucent pill; the trash icon sits left of the close icon on hover. The leading **Open Video** tile uses the same footprint and `rp-recent-card` chrome plus dashed border styling in `theme_continue_grid.css`; it activates `app.open` (same flow as the **Open Video** menu entry).
- Snackbar: pill-shaped at the bottom; auto-hide after 10 s; remove and trash share one session LIFO stack; Undo snapshots include watch-later sidecar bytes plus the full media row; trash entries also store the `Trash/files/…` path for untrash.
- `back_to_browse` clears the session undo stack except for trash (so the snackbar can offer untrash).
- Length and progress: write libmpv `duration` and `time-pos` to the DB on file switch and window close (no `ffprobe`); fall back to watch-later (`start=` / `# path`) before showing 0%.
- Thumbnails: `vo=image` libmpv with high-resolution seeking off; scale to ~480 px wide with `force_original_aspect_ratio=decrease`; JPEG quality ~82; video-only player with no audio / subtitles / external autoload / scripts / resume; loop-filter skipping only.
- Acceptance (manual): with ≥3 valid history entries, launch with no args → Open tile plus three cards in correct order, percentages match reopen behaviour, click loads + seeks. Empty history → browse strip shows Open tile only. With a CLI file, this grid is not the first view.
- Out of scope (v1): editing history order, hiding entries, streaming-art thumbs for remote URLs.
