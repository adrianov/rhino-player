# Keyboard, mouse, and shortcuts

**Name:** Input and keyboard shortcuts

**Implementation status:** In progress (Space toggles pause in app, not full mpv forwarding yet)

**Use cases:** Power users keep mpv muscle memory; casual users can view or override keys; mouse maps match typical player expectations.

**Short description:** Forward keys to mpv with GTK accelerator conflicts avoided; custom `input.conf` in the config dir; built-in default bindings; optional Adwaita Shortcuts window listing effective bindings; mouse map for buttons and double-click; scroll maps to WHEEL keys.

**Long description:** Load internal bindings from a memory `input.conf`, then optional user `input.conf`. Key events in capture phase translate to mpv’s `keypress`/`keyup` with modifier and key remaps. Mouse button gestures send `keypress` for mapped `MBTN_*` commands. Scroll on overlay sends wheel key combos. Optional: `?` for shortcuts dialog populated from `input-bindings` property.

**Specification:**

- Do not pass keys that match registered app accelerators to mpv.
- **Space** toggles play/pause via the mpv `pause` property when the main window is focused (and the player is ready).
- **m** toggles mute (see [Audio volume / mute](22-audio-volume-mute.md)).
- **Up** / **Down** adjust volume by 5% (clamped).
- **q** and **Ctrl+Q** run `app.quit` (resume snapshot, then exit).
- **Enter** and **KP_Enter** toggle fullscreen (see [Window behavior](17-window-behavior.md)).
- Escape leaves fullscreen first; a **second** Escape (when not fullscreen) stops playback, saves resume/DB, and returns to the **recent-videos** card screen when there is history (same as empty launch; otherwise the overlay stays hidden and the status invites opening a file).
- Tab focuses UI chrome temporarily.
- Document location of `input.conf` under `~/.config/rhino/` (TBD), following XDG.
