# Move current file to trash

**Name:** Move to Trash (app menu + shortcut)

**Implementation status:** Done (`app.move-to-trash`, gio [File::trash](https://docs.gtk.org/gio/method.File.trash.html), session **Undo** with `trash_xdg` to locate/restore the Freedesktop trash `files/…` entry; `back_to_browse` skips clearing the session undo stack for this action so the snackbar can offer **untrash**; the browse transition otherwise matches **Close Video** ([02](02-application-shell.md)))

**Use cases:** Remove the playing local file the same way the file manager does, without opening a separate app.

**Short description:** Main menu item **Move to Trash**; enabled only for a **local** file in playback (not the continue grid, not streams). Uses the session **Trash** via GLib. Captures a snapshot, clears **resume** and **continue** for that path, then usually leaves playback the same as **Close Video**, with an optional **Undo** that **untrash**es and restores the snapshot when the trashed `files/…` entry is found.

**Long description:** On success, the file at the current path is moved to the Freedesktop trash (same as **Move to Trash** in Nautilus). The action is a no-op with log on failure. Remote URLs and missing files are out of scope for this action. After a successful trash, the app clears [watch_later/DB] resume for the path, removes it from the continue **history** list, and returns to the continue grid (or empty state) with the usual [mpv] `stop` idle chain. The session **Undo** bar can restore the file from `~/.local/share/Trash` (Freedesktop layout: locate `files/…` + remove matching `.trashinfo`), put it back on the path it had before trash, and re-apply the same pre-trash `watch_later` / `media` snapshot as for **remove from list** (see [21-recent-videos-launch](21-recent-videos-launch.md)). If the trashed file cannot be located in trash (unusual), no undo token is pushed.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Move current file to trash
  Scenario: Trash during playback returns to browse with undo chance
    Given a local regular file path is loaded with chrome visible (grid hidden)
    When Move to Trash succeeds via menu or Delete shortcut
    Then the file lands in Freedesktop trash, resume clears, history removes the path
    And navigation matches Close Video except undo stack may retain trash undo entry

  Scenario: Guardrails for streams and grid
    Given playback is a URL stream or the continue grid covers the stage
    When the user attempts Move to Trash or Delete
    Then the action stays disabled without destructive calls

  Scenario: Undo restores file and snapshots when trash entry exists
    Given trash wrote files/… entry discoverable via trash_xdg
    When Undo activates within snackbar timing rules
    Then untrash restores original path plus watch_later/media snapshot semantics per recent grid doc
```

- **GAction** `app.move-to-trash` on [adw::Application]; **enabled** when the player is ready, the **continue** overlay is **hidden**, and [local_file_from_mpv] points at an existing **regular file** on disk; otherwise **disabled**.
- **Menu:** [gio::Menu] entry **“Move to Trash”** after **Close Video** (or next to it in the main menu), before the **Preferences** submenu. On the **continue** grid, each card (hover) shows a **trash** control to the **left** of “remove from list” when the path is a real **file**; [gtk::gio::File::trash] then [media_probe::remove_continue_entry] (or equivalent), then **refill**; the **Undo** bar may show **moved to trash** (same LIFO as **remove from list** when a matching `Trash/files/…` entry is found under XDG trash).
- **Shortcut** [Delete] (and **KP_Delete** for the keypad) — same affordance as many file managers. Does **not** use Shift+Delete (often “delete permanently” elsewhere).
- **Implementation** [gtk::gio::File::trash] with [None] cancellable; on **Ok** run `media_probe::capture_list_remove_undo` (snapshot) before trash, `media_probe::remove_continue_entry` after, push a **Trash** entry on the session undo stack when `trash_xdg::find_trash_files_stored_path` locates the `files/…` copy, then the same [back_to_browse] as **Close Video** but **without** clearing the session undo stack, then the undo bar + 10s dismiss timer. On **Err** log to stderr; do not clear resume or change UI state.

**Acceptance (manual):** With a local file playing, main menu **Move to Trash** and **Delete** move the file to Trash and return to the continue list; the original path is gone from the filesystem root (visible in the Trash), and **Undo** (when the snackbar is shown) restores the file, resume, and list entry. With no file or a stream URL, the item and shortcut are inactive.

**Related:** [Open files](06-open-and-cli.md), [Input shortcuts](13-input-shortcuts.md), [Application shell](02-application-shell.md).
