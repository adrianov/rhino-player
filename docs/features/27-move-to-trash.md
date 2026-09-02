# Move current file to trash

---
status: done
priority: p1
layers: [ui, storage, persistence]
related: [02, 13, 21, 34]
---

## Use cases
- Remove the playing local file the same way the file manager does, without opening another app.

## Description
A main-menu item **Move to Trash** and the **Delete** / **KP_Delete** shortcut move the playing file to the platform's trash. The action is enabled only when a local regular file is loaded and the continue grid is hidden. After a successful trash, the app clears watch_later / DB resume for that path, removes it from continue history and the media files catalog, drops it from the continue strip, and otherwise behaves like **Close Video** (see [02-application-shell](02-application-shell.md)). A session **Undo** can untrash the file and restore its snapshot.

## Behavior

```gherkin
@status:done @priority:p1 @layer:storage @area:trash
Feature: Move current file to trash

  Scenario: Trash during playback returns to browse with undo affordance
    Given a local regular file is loaded with chrome visible and the continue grid is hidden
    When the user activates Move to Trash via menu or Delete
    Then the file lands in the platform's trash
    And resume and continue history are cleared for that path
    And the catalog no longer holds that path
    And the continue strip does not show a card for it
    And the app returns to the continue grid like Close Video
    And the session undo stack retains a Trash entry when the trashed copy can be located

  Scenario: Disabled for streams and on the continue grid
    Given playback is a URL stream or the continue grid covers the stage
    When the user attempts Move to Trash or Delete
    Then the action remains disabled and no destructive call runs

  Scenario: Undo restores file and snapshot
    Given the trash entry is discoverable
    When the user activates Undo within the snackbar timeout
    Then the file is untrashed back to its original path
    And watch_later and media snapshots are restored per recent-grid undo rules

  Scenario: Trash failure leaves state untouched
    Given the trash call fails (permissions, missing file, full trash)
    When the action is invoked
    Then resume, history, and UI state are unchanged
    And the failure is logged
```

## Notes
- Menu / shortcut action id: `app.move-to-trash` (`Delete` / `KP_Delete`).
- **Linux:** `gio::File::trash`; Undo locates the copy under XDG `Trash/files` via `trash_xdg`.
- **macOS:** `NSWorkspace.recycleURLs` in `trash_macos` (Finder Trash / Dock). `NSFileManager.trashItemAtURL` and `gio::File::trash` only rename into `~/.Trash` and Finder often never lists the item.
- On success: `media_probe::capture_list_remove_undo`, then `trash_xdg::trash_local_file_for_undo` (platform trash + `forget_file` using the catalog key captured while the file still exists), then `media_probe::remove_continue_entry` / `recent_view::note_path_trashed` on that same key. Playing-file trash sets `skip_media_persist` before the platform move (Finder recycle pumps the GTK loop) and stops playback so browse-back cannot rewrite `media`. Continue-card hover trash uses `recent_view::card_trashed` (listing pin, catalog forget, lucky/search refill) and then the same undo stack.
- The browse transition matches **Close Video** but does not clear the session undo stack, so the snackbar can offer untrash.
- The trash control on continue cards lives in [21-recent-videos-launch](21-recent-videos-launch.md).
