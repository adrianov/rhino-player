# Recent list: "still watching" vs finished, thumbnails, remove + undo

---
status: research
priority: p2
layers: [ui, db, mpv]
related: [07, 21, 27]
---

## Use cases
- Keep the recent grid aligned with shows in progress, not a log of every file ever opened.
- Avoid misleading end-frame thumbnails after a file finishes.
- Let users drop entries with one click and undo accidents safely.

## Description
This is a **planning gate** for full "finished" semantics, DB cleanup rules, dismiss controls, and undo. Partial UI (card dismiss, undo bar) ships under [21-recent-videos-launch](21-recent-videos-launch.md). The scenarios below describe target behaviour; this file does not change runtime behaviour by itself.

## Behavior

```gherkin
@status:research @priority:p2 @layer:db @area:recent
Feature: Recent list semantics — finished vs in progress (planned)

  Scenario: Finished local file drops from continue list
    Given natural EOF fires once for a local file via the documented gate
    And no further sibling load replaces playback for that finish event
    When finish handling runs
    Then the finished path is removed from history
    And media cleanup follows the proposed cleanup rules without writing an end-frame thumbnail

  Scenario: In-progress playback stays visible until finished rules remove it
    Given the user paused mid-file or escaped back with partial progress
    When thumbnails refresh under the stale-position rules
    Then cards reflect stored percent
    And no end-frame thumbnail is captured for an unfinished path

  Scenario: Manual remove with undo affordance
    Given the remove control activates without a confirmation dialog
    When the entry leaves history per the proposed rules
    Then a snackbar offers Undo within the documented timeout
    And Undo restores history and snapshot data per the proposed rules

  Scenario: Finish plus sibling advance removes only the completed row
    Given the sibling queue loads the next file after EOF per 07-sibling-folder-queue
    When try_load records opens for the advancing pair
    Then the completed path is removed before the next-file history record runs
    And the new file appears once at the front of history

  Scenario: Auto-finish removal is silent
    Given a file finishes naturally
    When auto-removal runs
    Then no snackbar appears for that auto-removal
    And only manual remove or trash trigger an Undo snackbar

  Scenario: Finish does not delete watch-later sidecar by default
    Given a finished local file is removed from the continue list
    When media cleanup runs
    Then the watch-later sidecar on disk remains for v1
    And re-opening the file from a file manager still resumes per mpv defaults
```

## Notes

### Current behaviour today (codebase)

| Area | Behaviour |
|------|-----------|
| `history` table | Append / touch on open via `history::record` from `try_load` when `LoadOpts::record`. Ordered by `last_opened`; max 20 rows (`db::MAX_HISTORY`). |
| `media` table | `duration_sec`, `time_pos_sec`, `thumb_png` (+ mtime / `thumb_time_pos_sec`) updated from `record_playback_for_current`, `set_thumb`, grid `ensure_thumbnail`. |
| Sibling advance at EOF | `maybe_advance_sibling_on_eof` → `try_load` → `MpvBundle::load_file_path`, which calls `write_resume_snapshot`, `record_playback_for_current` for the still-loaded file (the one that just hit EOF), then `loadfile` the next. The finished file gets a playback row written with end-ish `time-pos` / `duration`; no `save_cached_thumb` in that path. |
| Back to grid (Escape) | Idle chain records playback state, paints DB-cached cards, then background backfill refreshes missing/stale thumbnails near the current continue position. |
| Stale / remove today | Missing file: grey card; click uses `on_stale` → `history::remove`. No general "remove seen file"; no undo. |

### Proposed cleanup rules

1. **DB helpers**: `remove_history` already exists; add `remove_media_path` (or equivalent) used whenever an entry is removed for finish.
2. **Finish path**: in `maybe_advance_sibling_on_eof`, when `eof` resolves a `finished` path, run `history::remove` + clear media for it **before** `try_load(next)`; same when there is no next. Reuse `sibling_eof_done` to run once per file.
3. **Escape / quit**: do not capture thumbs in the quit path; only record playback state and let continue-bar backfill refresh missing/stale thumbs when shown.
4. **UI**: `✕` button on each card with `connect_clicked` and propagation control so card open does not fire.
5. **Toast wiring**: one global or per-window `ToastOverlay`; on Undo, `history::record` (and optional media restore from a small snapshot).

### Undo snapshot proposal

`UndoToken` (held 5–8 s on the main loop):

- `path` (canonical String).
- Optional `last_opened` / order hint (or just call `history::record(path)` to put it back on top).
- Optional restore of media: clone small structs from DB before delete (`duration`, `time_pos`, thumb BLOB) for a pixel-identical undo. v1 may skip and restore history only.

### Open questions

- **Watch-later on finish**: keep on disk (default v1) vs delete to forget resume — recommend keep.
- **Toast for auto-remove**: noisy; recommend silent for auto-finish, toast only for manual ✕ and trash.
- **100% before remove**: deleting media at finish avoids a flash of 100% on the grid — preferred.

### References

- [GNOME HIG — Patterns](https://developer.gnome.org/hig/patterns/feedback/) (toasts, undo)
- [libadwaita — Toast / ToastOverlay](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/) — match the crate version in this repo.

This document is a planning gate. Do not implement large behaviour changes without updating [21-recent-videos-launch](21-recent-videos-launch.md) in lockstep and aligning its scenarios with the changes here.
