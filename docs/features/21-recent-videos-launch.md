# Recent videos grid on empty launch

---
status: done
priority: p1
layers: [ui, persistence, storage]
related: [03, 06, 13, 14, 17, 18, 23, 27, 33, 34]
---

## Use cases
- Launch from the icon and resume what you were watching with one click.
- See up to five recent files at a glance, with a thumbnail and progress.
- See a title’s resolution class (1080p, 720p, 2160p, …) when pointing at a card.
- Drop entries from the list (with undo) when you no longer want to resume them.

## Description
On empty launch (no CLI paths, no other "open this first" path takes over the first paint), the main content shows a row that always begins with **Open Video** (same workflow as choosing a file from the main menu). Up to **five** continue cards follow in most-recently-opened order when history entries exist; when history is empty, only this tile appears. Each history card has a thumbnail (cover style), a human-readable title derived from the last path segment (release-style dots, season/episode markers, and technical tags collapsed — no ellipsis), a thin progress bar with numeric percent, and **Remove** / **Move to Trash** controls on hover. Hovering also shows a resolution class tag (1080p, 720p, 2160p, …) after the played-percent label when the persistent store has a picture size or the last path segment carries that tag.

Clicking a history card loads that file and unpauses, even if watch-later had stored a paused session. The first history card may be warm-preloaded paused behind the grid on launch; hovering another card updates the seek bar from that card’s stored length and resume position without loading the file. Activating a card (click or Space) hides the grid and reveals playback after a short reveal delay. Returning to the grid keeps the current file paused for warm reuse when the continue strip stays visible (including empty history while using this launch pattern). Playback stops when browsing back hides the strip (no boot-file launch paths).

History is durable, deduplicated by canonical path, capped at 20 entries (showing five), and drops missing files on load—except when an incomplete download path is gone and a finished file with the matching name (without the incomplete-download suffix) sits in the same folder: the persistent store then records that finished path for the entry and keeps it on the list. Thumbnails are WebP BLOBs in the SQLite `media.thumb_webp` column, refreshed in the background by a headless libmpv decode near the stored continue position (`screenshot-raw` → WebP encode → DB, no temp files). Remove and Move to Trash share a session **LIFO undo stack** with a 10 s snackbar that sits just under the card strip without moving the strip or the search box.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui @area:recent
Feature: Recent videos grid on empty launch

  Background:
    Given the persistent store is available

  Scenario: Continue strip appears on empty launch with valid history
    Given the first window is shown with no CLI paths and no session takeover
    And history contains at least one valid local entry
    When the window paints
    Then an Open Video tile is visible ahead of recent entries
    And up to five history cards appear after it most-recent-first
    And each history card shows a thumbnail, human-readable title from the path segment, and percent progress

  Scenario: Empty history still shows the continue strip with Open Video
    Given the first window is shown with no CLI paths and no session takeover
    And history is empty
    When the window paints
    Then the continue strip is visible with one primary tile
    And activating that tile chooses a media file through the same flow as opening from the main menu
    And no thumbnails from history appear beside it

  Scenario: Clicking a card opens and unpauses
    Given a continue card is visible and references a local file
    And the persistent store holds a resume position past the start for that file
    When the user activates the card
    Then loadfile completes for that path
    And playback position is at or near the stored resume position
    And playback is running
    And the grid hides after the warm-reveal delay

  Scenario: Warm preload reveal on Space
    Given the recent grid is visible and the first card is warm-preloaded paused
    And the persistent store holds a resume position past the start for that file
    When the user presses Space
    Then after the warm-reveal delay the grid hides, chrome reveals, the window presents, and playback is running
    And playback position is at or near the stored resume position

  Scenario: Hover updates the seek bar from stored continue info
    Given the recent grid is visible and the playback engine is ready
    And a continue card references a local file with a stored length and resume position
    When the pointer enters that card
    Then the seek bar shows that card’s stored length and resume position
    And that file is not loaded into the playback engine from the hover alone
    And stored resume positions for any hovered files remain unchanged in the persistent store

  Scenario: Hover shows a resolution tag after played percent
    Given a continue card references a local file
    And the persistent store holds a picture size of 1920 by 1080 for that file
    When the pointer enters that card
    Then a quality tag reading 1080p appears after the played-percent label
    And the quality tag hides when the pointer leaves the card

  Scenario: Hover resolution tag from the path when size is unknown
    Given a continue card references a local file whose last path segment includes a 720p tag
    And the persistent store has no picture size for that file
    When the pointer enters that card
    Then a quality tag reading 720p appears after the played-percent label

  Scenario: Hover omits resolution when quality is unknown
    Given a continue card references a local file with no stored picture size
    And the last path segment has no resolution tag
    When the pointer enters that card
    Then no quality tag appears after the played-percent label

  Scenario: Blu-ray continue card opens and plays
    Given a continue card references a Blu-ray disc folder
    And the persistent store holds a resume position past the start for that disc
    When the user activates the card
    Then playback is running
    And playback position is at or near the stored resume position
    And the seek preview shows the same title as the open disc

  Scenario: Card footprint stays landscape when a thumbnail is portrait
    Given the continue strip shows several cards
    And at least one card’s thumbnail is taller than it is wide
    When the strip lays out
    Then every card keeps the same shared landscape footprint
    And that tall thumbnail is cropped to cover its card without changing sibling card sizes

  Scenario: Card title omits incomplete download wrappers
    Given a continue card references a local file whose basename ends with an incomplete-download suffix and a long download id
    When the continue strip paints
    Then the card title shows the humanized media name without that suffix or id

  Scenario: Card title omits surround-sound tech tags
    Given a continue card references a local file whose last path segment includes a surround-sound channel tag
    When the continue strip paints
    Then the card title shows the humanized media name without that tag

  Scenario: Incomplete download entry adopts the finished file
    Given a continue history entry references an incomplete download path that is gone from disk
    And a finished media file with the matching name without that incomplete-download suffix sits in the same folder
    When the continue list loads
    Then the persistent store records the finished file path for that entry
    And the finished file appears on the continue strip
    And the stored resume position for that entry is kept

  Scenario: Remove from list with undo
    Given a card shows a remove control on hover
    When the user activates remove
    Then the entry is removed from history without confirmation
    And the watch-later sidecar and SQLite resume for that path are cleared
    And a snackbar offers Undo for 10 seconds
    And dismissing the snackbar discards the undo for that entry

  Scenario: Move to Trash with undo
    Given a card represents an existing local file
    When the user activates Move to Trash
    Then the file is moved to the system Trash
    And history and resume are cleared for that path
    And that card leaves the strip
    And the catalog no longer holds that path
    And the snackbar offers Undo when the trashed copy is locatable
    And Undo restores the file plus the captured watch-later and media snapshot

  Scenario: Undo snackbar does not move the strip or search
    Given the continue strip and its search box are visible
    When the user removes or trashes a card so the undo snackbar appears
    Then the snackbar appears just under the card strip
    And the card strip and the search box stay in the same place

  Scenario: Finished file leaves continue list on switch
    Given a local file reaches natural end or the near-end window while another file loads
    When sibling advance or a user switch runs
    Then the finished path leaves history
    And its resume position is cleared

  Scenario: Credits skip clears continue entry
    Given a long local file is playing past the watched threshold but has not ended
    When the user opens another local file
    Then the previous file leaves the continue list
    And its resume position is cleared

  Scenario: Incomplete download stays when switching mid-file
    Given an incomplete download is playing past the watched threshold of its reported length
    When the user opens another local file
    Then the incomplete download stays on the continue list

  Scenario: Padding double-click toggles fullscreen
    Given the grid is visible with spacer padding around the card row
    When the user double-clicks primary on the spacers (not on a card or the undo bar)
    Then fullscreen toggles like the main video surface

  Scenario: Stale card shows greyed art and click removes
    Given a history entry exists for a path that no longer resolves on disk between refreshes
    When the user clicks the stale card
    Then the entry is removed from history via the on_stale path

  Scenario: Thumbnails refresh near stored continue position
    Given a card has a stored thumbnail that is not fresh for the current continue position
    And that continue position is past the start of the title
    When the continue strip is shown
    Then the card still shows the previous thumbnail image
    When the background backfill finishes a new thumbnail near the current continue position
    Then the card shows the new thumbnail image
    And the generic video placeholder does not appear between the old and new image

  Scenario: Thumbnail backfill does not interrupt horizontal scrolling
    Given the continue strip shows several cards
    And the user is scrolling the strip horizontally
    When background thumbnail work finishes for one of those cards
    Then that card's still updates in place
    And the strip does not rebuild or jump its scroll position

  Scenario: Existing thumbnail is kept at zero progress
    Given a card already has a stored thumbnail
    And the continue position for that title is still at the start
    When background backfill runs for that card
    Then the stored thumbnail is kept without decoding a new still from the start

  Scenario: Cards without any thumbnail show a placeholder
    Given a card has never had a thumbnail stored
    When the continue strip is shown
    Then the card shows a generic video placeholder until a thumbnail is ready

  Scenario: Continue thumbnail matches resume position
    Given a file has a stored continue position between keyframes
    When background backfill generates a thumbnail for that position
    Then the captured frame reflects that continue position rather than only the nearest earlier keyframe

  Scenario: Dark scene still gets a thumbnail
    Given a file whose stored continue position shows a mostly dark scene
    When background backfill generates a thumbnail for that position
    Then a thumbnail of that dark frame is stored and shown on the card
    And the generic video placeholder is not shown for that card

  Scenario: Uniform-color still is retried later
    Given background thumbnail capture at the continue position yields a mostly flat color frame
    # solid fill, single-hue gradient, lightly textured color board, or solid black
    And a later time in the same file shows more picture detail
    # later times at doubling intervals from the first still
    When background backfill finishes
    Then the card stores and shows the more detailed later frame
    And the flat color frame is not kept as the card thumbnail

  Scenario: Opening black still is stepped past
    Given a file whose first seconds are a solid black picture
    And a later time in the same file shows more picture detail
    And the continue position is still at the start
    When background backfill finishes
    Then the card stores and shows the more detailed later frame
    And the solid black frame is not kept as the card thumbnail

  Scenario: Thumbnail crops baked-in black strips
    Given a file whose frames contain detectable black strips around the picture
    When background backfill generates a thumbnail for that file
    Then the stored thumbnail shows the picture without those black strips

  Scenario: Unparseable file leaves the continue strip
    Given a continue or result card is shown for a local file
    When background thumbnail work cannot parse that file
    Then that card leaves the strip
    And the Open Video tile stays
```

## Notes
- Store: SQLite `history` and `media` tables in `~/.config/rhino/rhino.sqlite` (user config directory).
- Trigger: empty CLI args; first paint follows this grid and CLI rules in [06-open-and-cli](06-open-and-cli.md).
- Deduplication: opening a path moves it to the front; capacity 20, display 5; `history::load` drops missing files, or adopts a finished Direct Connect sibling for a gone `*.dctmp` via `finished_download_path` + `db::rekey_continue_path` (history + media keys; on conflict keep the finished path and prefer the incomplete row’s media/resume). Entity-key dedupe must not delete the kept finished key when both incomplete and finished rows appear in one load.
- Card UI: each card shares one **16:9** footprint sized as if the strip were full (Open tile + five history slots) so a short list or search hit does not inflate tiles; width is clamped between a minimum and maximum (`card_dims::CARD_*` / `sync_card_sizes`). Thumbnail display uses cover style; `thumb_texture` (`grid_cover`) cover-crops WebP decode to 16:9 and builds the card picture so portrait/square stills cannot raise the row (GTK `AspectFrame` alone does not cap measure). Cards use start valign in the strip. Title and progress sit in a soft bottom gradient overlay; the percentage is a small translucent pill; on hover a matching quality pill (`rp-recent-quality`) follows it when `quality_tag_for` has a label (stored `media.decode_w`×`decode_h` via `db::media_decode_size`, else a path-segment tag such as `1080p` / `4K`). **Move to Trash** sits left of **Remove** on hover (shared `rp-recent-action` chrome in `theme/continue_grid.css`). Remove/Trash strip reflow is deferred to a GLib idle (`schedule_refresh_continue_cards`) so cards are not destroyed inside their own click/gesture handlers. Card hover calls `transport_sync_warm_browse` via `warm_hover_hooks` (seek bar from `ContinueGridCache` snap — no hover `loadfile`). While the continue grid is visible and warm hover is active, `MpvBundle::skip_media_persist` blocks SQLite `media` writes (resume, decode size, smooth ME budget); close / back-from-playback / quit after real playback use `save_playback_state_for_close`. The card title uses `human_media_title` on the basename (Transmission-style cleanup; window title uses the same helper). Surround-sound channel tokens are in `TECH_TAGS` (`src/human_media_title/tech_strip.rs`). Incomplete Direct Connect names (`name.ext.<id>.dctmp`) are peeled in `download_temp.rs` before the usual tag cleanup. The leading **Open Video** tile uses the same footprint and `rp-recent-card` chrome plus dashed border styling in `theme/continue_grid.css`; it activates `app.open` (same flow as the **Open Video** menu entry).
- Snackbar: pill-shaped overlay just under the card strip (`GtkOverlay` on the bottom spacer in `strip_stack.rs` — not a vbox slot, so the strip and search row do not reflow); auto-hide after 10 s; Remove and Move to Trash share one session LIFO stack; Undo snapshots include watch-later sidecar bytes plus the full media row; Trash entries also store the platform trash path for restore. The open-failure notice pill ([06](06-open-and-cli.md)) shares that overlay band.
- `back_to_browse` clears the session undo stack except for Trash (so the snackbar can still offer restore).
- Continue clear on switch: `is_continue_done` (`media_probe`) — natural end / last `NEAR_END_SEC`, or past `CONTINUE_DONE_FRAC` when duration exceeds `CONTINUE_DONE_MIN_SEC` (credits skip via **Next**). Fraction gate skipped for incomplete downloads (`*.dctmp`) and multi-part DVD titles. Warm preload never clears.
- Length and progress: write libmpv `duration` and `time-pos` to the DB on file switch and window close (no `ffprobe`); fall back to watch-later (`start=` / `# path`) before showing 0%. When mpv reports `bd://`, keys use `shell_media_path` + `me_budget_shell_path` (disc root), same as history; resume seek and warm-hit matching also use `media_probe::mpv_matches_open_target` (not raw mpv `path`, which is not a filesystem path on Blu-ray). **`mpv_warm_hit_ready`** matches only mpv’s **local** open path (`mpv_local_open_path`) — never `me_budget_shell_path`, which hover updates via `last_path` / `transport_sync_warm_browse` (seek bar only; activate still `loadfile`s) (otherwise a `bd://` title falsely warm-hits the hovered DVD). Warm `loadfile` skips outgoing SQLite snapshots (`warm_preload`); near-start mpv reads must not overwrite an existing resume (`db::set_playback`, `media_probe::NEAR_END_SEC`). Hover only updates `last_path` + seek/clocks via `continue_snap_for_browse` / `browse_pause_snap` (strip cache, else one-row `resume_pos` + `media_duration_sec`). Startup still warm-preloads the first continue file (`run_continue_warm_preload`); at most one startup/activate `loadfile` uses `WarmPreloadGate` / `warm_preload_notify_loaded` / path-settle debounce / 4s watchdog. Card click / Space warm reopen is a hit only when mpv’s open title already matches the target (`mpv_warm_hit_ready`). While the continue grid is visible, transport snap reads `last_path` (hover/card intent), not `me_budget_shell_path`. `card_data_list` reads resume/duration once per grid fill; `ContinueGridCache` (`ContinueSnap` per canonical path) is refreshed in `fill_row` and shared with transport (`continue_grid_cache_lookup` in `read_transport_state` — no per-tick SQLite). `last_path` is updated via `transport_sync_warm_browse` on hover/load start. Rapid hover uses `warm_file_gen` so stale `FileLoaded` idles do not resume the wrong title. Modules: `warm_preload_idle.rs`, `warm_preload_path.rs`, `preload_continue_and_run.rs`.
- Thumbnails: headless libmpv on **worker threads** (`THUMB_WORKERS` in `live_card/thumb_backfill.rs` — several captures at once so a Lucky / search handful of never-watched files is not strictly serial; `ThumbBackfill` / `schedule_thumb_backfill`); only the paths painted on the continue / search / lucky strip are backfilled ([33](33-continue-sibling-search.md)). A loadfile error or engine demux failure (`GridThumb::Unparseable`) forgets the catalog path and rebuilds the strip without that card (seek/frame/demuxer-wait timeouts stay on the card). Ready paths coalesce in a `Send` inbox; the first push of a burst schedules one `glib::MainContext::invoke` (same hop as mpv event drain — not a refill poll timer). The main-thread flush applies stills in place via `apply_ready_thumbs` (no `fill_row`, scroll stays put). `vf=scale=640:…` downscale in mpv (≈ card max width); per-poll `frame-step` + `screenshot-raw video` (mpv ≤0.38: flags only, always `bgr0`; newer mpv adds an optional format arg — omit it for compatibility); before WebP encode, packed-pixel bar crop (`black_bars::detect_packed_crop`) removes baked-in black strips when present (skipped on mostly-black frames so dark scenes stay intact); zenwebp reads the mpv `&[u8]` slice in place (`PixelLayout::Bgra8`, `with_stride` for row padding) and only allocates the WebP output; `mpv_free_node_contents` runs after encode returns. Quality ~82 (`GRID_THUMB_WEBP_Q`), encode **method 0** (fast). Mostly-black captures are held back briefly; a frame that stays dark across the stability window (`DARK_STABLE_POLLS`, `src/media_probe/thumb_screenshot_raw.rs`) is a real dark scene and is kept when it still has picture detail. Almost-uniform fills (`FLAT_STABLE_POLLS` / `thumb_webp_is_flat_fill` / `rgb_samples_mostly_flat` — few RGB buckets, or one chromatic primary with limited bucket spread so solid fills / single-hue gradients / light mesh / solid black count without treating a detailed mono-tinted scene as flat) trigger exponential forward seeks (`flat_nudge_seeks` in `thumb_vo_image_flat_nudge.rs`; exact seek so a keyframe snap does not collapse the steps; cap from demuxer duration when known) before storing; `grid_thumb_flat_capture` accepts a still-flat BLOB afterward so workers do not respin. Until that capture runs, cached flat BLOBs are rejected and display may show a placeholder. Video-only player with no audio / subtitles / external autoload / scripts / resume; loop-filter skipping only. All formats: `loadfile` then seek — resume-position stills use `absolute+exact` with `hr-seek=yes`; unstarted titles (`resume` still 0, start fallback) use `absolute+keyframes` so first-open and Lucky cards do not wait on an exact decode. A small non-zero resume stays exact. DVD titles: `still_target_from_global` picks the resume chapter `.vob` and local offset (same timeline as preview/resume); chain-head `.vob` waits for stretched duration before seek (same cap as seek-bar preview). Cache key includes `thumb_load_path` so a thumb from the wrong VOB is not reused. UI uses `thumb_backfill_satisfied` to skip workers (fresh cache **or** any stored still while resume is still at 0 — no re-decode at the 2s start fallback); `cached_thumbnail_for_display` falls back to `stored_thumb_webp` so cards keep the last picture until a new BLOB is written (placeholder only when no thumb was ever stored). Seek-bar hover preview still uses keyframe seeks for scrub latency ([18-thumbnail-preview](18-thumbnail-preview.md)).
- Acceptance (manual): with ≥3 valid history entries, launch with no args → Open tile plus three cards in correct order, percentages match reopen behaviour, click loads + seeks. Empty history → browse strip shows Open tile only. With a CLI file, this grid is not the first view.
- Out of scope (v1): editing history order, hiding entries, streaming-art thumbs for remote URLs.
