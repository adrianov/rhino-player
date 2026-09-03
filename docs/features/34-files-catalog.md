# Media files catalog

---
status: wip
priority: p1
layers: [persistence, storage, playback]
related: [06, 07, 21, 27, 31, 33]
---

## Use cases
- Keep one durable catalog of every local video the player has seen (open and shallow sibling neighbours) so continue, search, and later lists share one identity.
- Record technical details only when a feature needs them (continue card, Smooth budget, transport), then reuse what is already stored.
- Remove a catalog entry when the app uses a path and finds the file missing, or when a still cannot be parsed from it — not via a background library scan.

## Description
The persistent store holds a **media files catalog**: one entry per known local video (or multi-part disc title). Opening a local file to play **registers** that path and, once per session for that file, also registers its shallow neighbours: other videos in the same folder, and videos sitting directly in sibling folders under the same parent — without walking up the tree or into nested folders inside those siblings. The continue list (and other lists later) **points at** catalog entries instead of maintaining a separate set of paths.

Technical facts — total length, decode size, source frame rate, thumbnail image, container modification time, and similar — begin unset. The first feature that needs a fact and can learn it writes it once; later reads use the store. Playback preferences that describe the user’s session rather than the file itself (resume position, last sound/subtitle choice, fill-screen, Smooth budget) live on a linked per-path playback-state row that shares the same catalog identity. When continue painting or history load already finds a path absent on disk, that entry leaves the continue list and the catalog — the same absence checks used for stale continue cards today, without a dedicated “rescan library” pass. When background still capture cannot parse a catalogued file, that path leaves the catalog and the continue list the same way.

## Behavior

```gherkin
@status:wip @priority:p1 @layer:persistence @area:files-catalog
Feature: Media files catalog

  Scenario: Opening a video registers it in the catalog
    Given the user opens a local video that is not yet in the catalog
    When that video becomes the current playback entity
    Then the persistent store holds a catalog entry for it
    And technical facts may still be unset until something needs them

  Scenario: Opening a video registers same-folder neighbours
    Given a local folder holds several videos
    And the user has not opened any of them this session
    When the user opens one of those videos to play
    Then each video in that folder has a catalog entry
    And no length, still, or frame-rate facts are read from those files solely to register them

  Scenario: Opening a video registers videos in sibling folders
    Given a local folder holds a video
    And another folder beside it under the same parent holds videos
    When the user opens the first folder’s video to play
    Then videos that sit directly in that neighbouring folder have catalog entries
    And nested folders inside that neighbouring folder are not walked for catalog registration

  Scenario: Sibling catalog scan does not walk up
    Given a video sits in a folder whose parent is not the filesystem root
    And videos exist only under a different folder that is a sibling of that parent
    When the user opens that video to play
    Then those higher-tree videos are not added to the catalog by that open

  Scenario: Top-level folder open does not scan peer roots
    Given a video sits directly in a folder under the filesystem root
    And another folder under the filesystem root holds videos
    When the user opens that video to play
    Then videos under the other root-level folder are not added to the catalog by that open
    And other videos in the same folder still receive catalog entries

  Scenario: Sibling catalog scan runs once per opened file per session
    Given the user already opened a local video this session and its neighbours were registered
    When the user opens the same video to play again in the same session
    Then the player does not walk that folder tree again for catalog registration

  Scenario: Catalog scan skips a text file that shares a transport-stream suffix
    Given a folder listing finds a local file whose name uses the transport-stream suffix
    And the platform content type for that file is not a video type
    When those paths are collected for the catalog
    Then that path is not added to the catalog

  Scenario: Catalog scan keeps a transport-stream video
    Given a folder listing finds a local file whose name uses the transport-stream suffix
    And the platform content type for that file is a video type
    When those paths are collected for the catalog
    Then the persistent store holds a catalog entry for it

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

  Scenario: Unparseable file leaves the catalog
    Given a catalog entry exists for a local file that looks like a video
    When the player cannot parse that file to capture a still
    Then the catalog entry for that path is removed
    And the path is not on the continue list

  Scenario: Trashed file leaves the catalog
    Given a catalog entry exists for a local file
    When the user moves that file to the platform trash
    Then the catalog entry for that path is removed
    And the continue strip does not show a card for it

  Scenario: Playback preferences stay apart from file facts
    Given a video in the catalog has a stored resume position and track choices
    When technical facts on the catalog entry are updated
    Then resume position and track choices stay unchanged
```

## Notes
- **Shipped so far (path registry):** table `files (path TEXT PRIMARY KEY, discovered_at INTEGER)` in `db/history_files_catalog.rs` (history `#[path]` submodule); duration helpers in `db/history_media_duration.rs`; `ensure_file` / `list_file_paths` / `forget_file`; `record_history` registers the opened path. **Open-to-play sibling registration:** `ensure_open_siblings` (same module) runs from `try_load` when history is recorded — once per session per opened path key. Collects `list_videos_in_dir` for the file’s folder plus each peer folder under the same parent (no recurse into those peers; skip peer-folder walk when the parent is the filesystem root, and on macOS when the parent is `/Volumes` so other mounts are not scanned). Writes use one transaction of insert-or-ignore; bumps `files_catalog_epoch` when any row is new so `CatalogMem` reloads on the next ensure. Neighbour search and I'm Feeling Lucky ([33](33-continue-sibling-search.md)) load paths via `CatalogMem` (`list_file_paths`, invalidated by that epoch) and filter in memory. **Missing on disk** is owned by `media_probe/catalog_miss.rs`: `forget_missing` (adopt incomplete-download sibling or drop catalog + continue), plus thin callers `path_is_openable` / `drop_after_open_fail` / `dismiss_continue_path` / `ensure_listed_thumbnail` / continue `card_one`. Grid still capture that gets an engine load or demux failure (`GridThumb::Unparseable` in `media_probe`) calls `forget_file` and drops continue history; demuxer-wait timeouts stay; incomplete downloads and optical discs stay for unparseable stills. **Move to Trash** (card or playing file) captures the catalog key before the file leaves disk (`trash_xdg::trash_local_file_for_undo` → `forget_file`), then `recent_view::note_path_trashed` drops the neighbour-index row; playing-file trash forgets again after browse-back persist. Undo `history::record` registers the path again. The shared video suffix `ts` is also used by TypeScript sources: `is_video_path` / `list_videos_in_dir` keep a `.ts` file only when `ts_file_is_video` (`video_ext/ts_mime.rs`) says so — GIO `content_type_guess` on a short read (reject `text/*` / typescript / javascript) plus MPEG-TS sync `0x47` at 188-byte packets so an extension-only video MIME does not catalog a text program file.
- **Still planned:** lazy tech columns, continue pointing at catalog identity, moving tech off `media` — see scenarios above.
- **Tables (target):** `files` (catalog + lazy tech) vs existing `media` (playback state) vs `history` (continue membership). Same entity path key from `playback_entity::db_path_for` / `db::history_key`.
- **Out of scope for v1:** recursive library watchers, network URLs in the catalog, dedicated “rescan library” UI.
