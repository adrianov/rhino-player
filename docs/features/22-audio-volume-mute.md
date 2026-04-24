# Audio: volume, mute, persistence

**Name:** System volume, mute, and last level persistence

**Implementation status:** In progress

**Use cases:** Adjust loudness without leaving the app; one-click mute; match expectations from other desktop players and mpv; restore last used level on the next run.

**Short description:** Header control with a popover (horizontal level scale + Mute), scroll-wheel on the video to change volume, keyboard shortcuts, and `volume` / `mute` stored in the app database so the next launch matches the previous session (mpv remains the source of truth while playing).

**Long description:** A `MenuButton` with a symbolic icon (muted / low / medium / high) opens a popover containing a `Scale` for `0…volume-max` and a Mute `CheckButton` row. The existing transport poll updates the header when the user does not have the slider in focus (same pattern as seek). `EventControllerScroll` on the `GLArea` changes volume when the recent grid is not covering the view; scroll direction follows common “wheel down = lower volume” behavior. The SQLite `settings` table stores `master_volume` and `master_mute` strings, applied to mpv after the render context is created and written again on quit (before `commit_quit`).

**Specification:**

- `volume` and `mute` on libmpv stay consistent with the popover, scroll wheel, and keys.
- Popover shows the level numerically in the scale’s value label (`draw_value` = true) or a short percentage label; mute state does not change the stored volume value (unmute restores the prior level).
- **Scroll (video):** vertical scroll on `GLArea` adjusts volume by 5% per notched step (smoothed trackpads aggregate sensibly; small `dy` values change volume proportionally, clamped to range).
- **Keys (main window, player ready):** **↑** / **↓** nudge volume by 5% (clamped); **m** toggles mute. Do not add extra notifications for volume changes.
- On startup, if settings exist, apply `volume` and `mute` to mpv after `MpvBundle` creation. Defaults: 100% volume, not muted.
- On quit, persist current `volume` and `mute` to the DB before `commit_quit` runs `stop` (so values reflect real playback, not a forced idle state from quit).

**Current code:** `src/app.rs` (header control, GL scroll, key bindings), `src/db.rs` (`settings` + load/save), `src/mpv_embed.rs` (no extra hooks beyond existing property API).
