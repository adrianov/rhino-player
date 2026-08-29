# Media files catalog

---
status: planned
priority: p1
layers: [persistence, storage, playback]
related: [07, 21, 31, 33]
---

## Use cases
- Keep one durable catalog of every local video the player has seen (open, folder, sibling walk, neighbour search) so continue, search, and later lists share one identity.
- Record technical details only when a feature needs them (continue card, Smooth budget, transport), then reuse what is already stored.
- Remove a catalog entry only when the app uses a path and finds the file missing — not via a background library scan.

## Description
The persistent store holds a **media files catalog**: one entry per known local video (or multi-part disc title). Any open or directory listing that yields a playable path **registers** that path in the catalog if it is new. The continue list (and other lists later) **points at** catalog entries instead of maintaining a separate set of paths.

Technical facts — total length, decode size, source frame rate, thumbnail image, container modification time, and similar — begin unset. The first feature that needs a fact and can learn it writes it once; later reads use the store. Playback preferences that describe the user’s session rather than the file itself (resume position, last sound/subtitle choice, fill-screen, Smooth budget) live on a linked per-path playback-state row that shares the same catalog identity. When continue painting or history load already finds a path absent on disk, that entry leaves the continue list and the catalog — the same absence checks used for stale continue cards today, without a dedicated “rescan library” pass.

## Behavior

```gherkin
@status:planned @priority:p1 @layer:persistence @area:files-catalog
Feature: Media files catalog

  Scenario: Opening a video registers it in the catalog
    Given the user opens a local video that is not yet in the catalog
    When that video becomes the current playback entity
    Then the persistent store holds a catalog entry for it
    And technical facts may still be unset until something needs them

  Scenario: Directory listings register paths without reading technical facts
    Given a folder listing or neighbour search yields video paths
    When those paths are collected for display or navigation
    Then each path has a catalog entry
    And no technical facts are read from the file solely to register it

  Scenario: Continue list points at catalog entries
    Given a video is on the continue list
    When the continue strip paints
    Then that entry uses the catalog identity for the path
    And card technical fields come from the catalog when already set

  Scenario: Technical facts are filled on demand
    Given a catalog entry exists with total length unset
    When the app needs total length for that path (card, transport, or budget)
    Then the app learns the length once and stores it on the catalog entry
    And a later need for the same fact reads the store without reading the file again
    # Same pattern for decode size, source frame rate, and continue-grid thumbnail when those features ask

  Scenario: Missing file leaves catalog and continue list
    Given a continue entry points at a path already in the catalog
    When the continue strip or history load finds the file absent on disk
    Then that path leaves the continue list
    And the catalog entry for that path is removed
    And no separate full-library scan is required

  Scenario: Playback preferences stay apart from file facts
    Given a video in the catalog has a stored resume position and track choices
    When technical facts on the catalog entry are updated
    Then resume position and track choices stay unchanged
```

## Notes
- **Tables:** `files` (catalog + lazy tech) vs existing `media` (playback state: `time_pos_sec`, `aid`/`sid`/IFO slots, `fill_screen`, `smooth_me_budget_*`) vs `history` (continue membership + `last_opened`). Same entity path key from `playback_entity::db_path_for` / `db::history_key`.
- **`files` columns (initial):** `path` PK, `discovered_at`, optional cheap `source_mtime_sec` / size; nullable tech: `duration_sec`, `decode_w`/`decode_h`, `source_fps_hz`, `thumb_webp` + thumb meta (moved from today’s `media` length/decode/fps/thumb **facts**; resume stays on `media`). Codecs wait until a caller needs them.
- **Register call sites:** open/CLI/DnD, `list_videos_in_dir` consumers (sibling advance, folder open, neighbour search `scan_watch_later_dirs`), `history::record`. API sketch: `db::ensure_file(path)` (insert-or-ignore) and `db::file_tech_*` getters that fill on miss.
- **Forget call sites:** today’s absence paths (`history::load` prune, stale continue card, open that finds the file gone) → `db::forget_file(path)` clears `files` + `media` + `history` for that key (same idea as `remove_continue_entry`, extended to the catalog).
- **Migration:** create `files`; copy tech columns from `media` for existing paths; `ensure_file` for every `history` path; keep `media` for playback state only (drop migrated tech columns from `media` once callers move).
- **Out of scope for v1:** recursive library watchers, network URLs in the catalog, dedicated “rescan library” UI.
