# Move current file to trash

---
status: done
priority: p1
layers: [ui, fs, db]
related: [02, 13, 21]
actions: [app.move-to-trash]
---

## Use cases
- Remove the playing local file the same way the file manager does, without opening another app.

## Description
A main-menu item **Move to Trash** and the **Delete** / **KP_Delete** shortcut move the playing file to the Freedesktop Trash. The action is enabled only when a local regular file is loaded and the continue grid is hidden. After a successful trash, the app clears watch_later / DB resume for that path, removes it from continue history, and otherwise behaves like **Close Video** (see [02-application-shell](02-application-shell.md)). A session **Undo** can untrash the file and restore its snapshot.

## Behavior

```gherkin
@status:done @priority:p1 @layer:fs @area:trash
Feature: Move current file to trash

  Scenario: Trash during playback returns to browse with undo affordance
    Given a local regular file is loaded with chrome visible and the continue grid is hidden
    When the user activates Move to Trash via menu or Delete
    Then the file lands in the Freedesktop trash
    And resume and continue history are cleared for that path
    And the app returns to the continue grid like Close Video
    And the session undo stack retains a Trash entry when the trashed files/… copy can be located

  Scenario: Disabled for streams and on the continue grid
    Given playback is a URL stream or the continue grid covers the stage
    When the user attempts Move to Trash or Delete
    Then the action remains disabled and no destructive call runs

  Scenario: Undo restores file and snapshot
    Given the trash entry is discoverable via trash_xdg
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
- Implementation calls `gio::File::trash` with no cancellable. On success: `media_probe::capture_list_remove_undo`, then `media_probe::remove_continue_entry`, then push a Trash entry on the session undo stack when `trash_xdg::find_trash_files_stored_path` resolves the `files/…` copy.
- The browse transition matches **Close Video** but does not clear the session undo stack, so the snackbar can offer untrash.
- The trash control on continue cards lives in [21-recent-videos-launch](21-recent-videos-launch.md).
