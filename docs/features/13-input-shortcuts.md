# Keyboard, mouse, and shortcuts

**Name:** Input and keyboard shortcuts

**Implementation status:** Done for in-app shortcuts in `src/app/input.rs` (not: forwarding arbitrary keys to mpv from user `input.conf`, Shortcuts help window)

**Use cases:** Power users keep mpv muscle memory; casual users can view or override keys; mouse maps match typical player expectations.

**Short description:** Forward keys to mpv with GTK accelerator conflicts avoided; custom `input.conf` in the config dir; built-in default bindings; optional Adwaita Shortcuts window listing effective bindings; mouse map for buttons and double-click; scroll maps to WHEEL keys.

**Long description:** Load internal bindings from a memory `input.conf`, then optional user `input.conf`. Key events in capture phase translate to mpv’s `keypress`/`keyup` with modifier and key remaps. Mouse button gestures send `keypress` for mapped `MBTN_*` commands. Scroll on overlay sends wheel key combos. Optional: `?` for shortcuts dialog populated from `input-bindings` property.

**Specification:**

- Do not pass keys that match registered app accelerators to mpv.
- **Space** toggles play/pause via the mpv `pause` property when the main window is focused (and the player is ready).
- **Primary double-click** on the video view toggles fullscreen. **Secondary (right) single-click** on the video view toggles play/pause the same as Space (when a file with duration is loaded).
- **m** toggles mute (see [Audio volume / mute](22-audio-volume-mute.md)).
- **Up** / **Down** adjust volume by 5% (clamped).
- **q** and **Ctrl+Q** run `app.quit` (resume snapshot, then exit).
- **Ctrl+W** runs `app.close-video`: same as that post-fullscreen Escape path (return to the **continue / recent** grid and stop the current file) — **does not** quit the app. The **bottom** transport bar and main menu both expose **Close video** for a mouse-only path. The action is disabled when the grid is already showing or the player is not ready.
- **Delete** and **KP_Delete** run `app.move-to-trash` when a **local file** is playing (not streams); same as the main menu **Move to Trash** (see [27-move-to-trash](27-move-to-trash.md)). Disabled on the continue grid or for non-file sources.
- **Enter** and **KP_Enter** toggle fullscreen (see [Window behavior](17-window-behavior.md)).
- On the **continue / recent** grid, **double** primary click on the **empty** area (padding above, below, or beside the card row, not on a card) toggles fullscreen the same as on the video (see [Recent videos](21-recent-videos-launch.md)).
- Escape leaves fullscreen first; a **second** Escape (when not fullscreen) **pauses** at once (so sound stops) while the app finishes resume/DB work and returns to the **recent-videos** card screen when there is history, then `stop` runs in the main-loop idle chain (same as empty launch; otherwise the overlay stays hidden and the status invites opening a file).
- Tab focuses UI chrome temporarily.
- Document location of `input.conf` under `~/.config/rhino/` (TBD), following XDG.
