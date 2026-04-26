# Move current file to trash

**Name:** Move to Trash (app menu + shortcut)

**Implementation status:** Done (`app.move-to-trash`, [gio] `GFile` → `g_file_trash`, same [back_to_browse] path as [Close video](02-application-shell.md) after success)

**Use cases:** Remove the playing local file the same way the file manager does, without opening a separate app.

**Short description:** Main menu item **Move to Trash**; enabled only for a **local** file in playback (not the continue grid, not streams). Uses the session **Trash** via GLib. Clears **resume** and **continue** for that path, then leaves playback the same as **Close video**.

**Long description:** On success, the file at the current path is moved to the Freedesktop trash (same as **Move to Trash** in Nautilus). The action is a no-op with log on failure. Remote URLs and missing files are out of scope for this action. After a successful trash, the app clears [watch_later/DB] resume for the path, removes it from the continue **history** list, and returns to the continue grid (or empty state) with the usual [mpv] `stop` idle chain.

**Specification:**

- **GAction** `app.move-to-trash` on [adw::Application]; **enabled** when the player is ready, the **continue** overlay is **hidden**, and [local_file_from_mpv] points at an existing **regular file** on disk; otherwise **disabled**.
- **Menu:** [gio::Menu] entry **“Move to Trash”** after **Close video** (or next to it in the main menu), before the **Video** submenu. On the **continue** grid, each card (hover) shows a **trash** control to the **left** of “remove from list” when the path is a real **file**; same [gtk::gio::File::trash] → `clear_resume_for_path` → `history::remove` as above, then **refill** the row (not the “removed from list” **Undo** snackbar).
- **Shortcut** [Delete] (and **KP_Delete** for the keypad) — same affordance as many file managers. Does **not** use Shift+Delete (often “delete permanently” elsewhere).
- **Implementation** [gtk::gio::File::trash] with [None] cancellable; on **Ok** run [media_probe::clear_resume_for_path] and [history::remove] for the canonical file path, then the same [back_to_browse] context as the **Close video** action. On **Err** log to stderr; do not clear resume or change UI state.

**Acceptance (manual):** With a local file playing, main menu **Move to Trash** and **Delete** move the file to Trash and return to the continue list; the original path is gone from the filesystem root (visible in the Trash). With no file or a stream URL, the item and shortcut are inactive.

**Related:** [Open files](06-open-and-cli.md), [Input shortcuts](13-input-shortcuts.md), [Application shell](02-application-shell.md).
