# Thumbnails: seek bar preview

---
status: done
priority: p1
layers: [ui, input, playback, persistence]
related: [03, 04, 14, 21]
scope: portable
---

## Use cases
- Scrub the timeline visually before seeking, especially on long local files.

## Description
Hovering over the seek bar shows a framed video preview and the corresponding playback time. Moving along the bar updates the preview without changing the current playback position.

The preview can be disabled in Preferences. It is available for media the application can preview locally and does not interrupt playback.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui @area:preview
Feature: Thumbnails: seek bar preview

  Scenario: Show a preview for local media
    Given seek bar preview is on
    And locally previewable media is open
    When the user hovers the seek bar at any position
    Then a thumbnail above the bar shows the video at the hovered time
    And the preview shows the formatted hover time

  Scenario: Keep unavailable media unchanged
    Given the open media cannot be previewed locally
    When the user hovers the seek bar
    Then no preview appears
    And playback remains unchanged

  Scenario: Follow rapid pointer movement
    Given a preview is showing above the seek bar
    When the user moves the pointer quickly to a new position
    Then the preview shows the latest hovered time
    And an older hover does not replace it

  Scenario: Respect the preview preference
    Given seek bar preview is off
    When the user hovers the seek bar
    Then no thumbnail appears
    And transport remains usable

  Scenario: Leaving the seek bar restores chrome
    Given the preview is showing above the seek bar
    When the pointer leaves the seek bar
    Then the preview is no longer shown
    And the window chrome arrangement matches the layout before the hover

  Scenario Outline: Show the preview in every window mode
    Given seek bar preview is on
    And locally previewable media is open in "<mode>"
    When the user hovers the seek bar
    Then a thumbnail above the bar shows the video at the hovered time

    Examples:
      | mode          |
      | normal window |
      | full screen   |
```

## Notes
- Settings: SQLite `seek_bar_preview` defaults to **on**; toggled from main menu Preferences (gio stateful action `seek-bar-preview`).
- Hover time is `(x / width) * bar_upper` capped by [seek_bar_label_time]. Pointer release on the seek bar (trough or thumb drag) seeks the main player to that hover time, not the raw GtkRange thumb value; preview off falls back to capped thumb time ([`seek_wiring`](../../src/app/seek_wiring.rs)).
- Preview **`GtkFrame`** on **`outer_ovl`** above the bottom bar; positioned from seek-bar pointer x; the preview **`GLArea`** is realised before first show.
- Thumbnail sizing follows the source aspect and the bounds in `state_and_vo_pump.rs`.
- Motion coalescing uses `PREVIEW_DEBOUNCE`; the debounce and frame pump run at default GLib priority.
- The `Progress Bar Preview` row is the only preview-related preference; no separate preferences window.
- Recent grid thumbnails use `screenshot-raw` plus DB WebP cache via `media_probe` / `thumb_texture`; this feature does not feed the grid.
- Load selection and decode limits are owned by `preview_media_load.rs`; the separate `MpvPreviewGl` never seeks the main player. Optical-media mapping is delegated to the playback entity and timeline modules.
- Leaving the bar hides the overlay with `set_visible(false)` but keeps the cached target. Reopen renders a warm frame immediately; `need_load` reloads after GL context loss. Main-media changes clear the target without replacing the preview GL context.
- Debug: `[rhino] preview:` lifecycle and failure lines are always printed; `RHINO_PREVIEW_DEBUG=1` adds frame-pump trace.
- UI: preview is a **`GtkFrame`** overlay on **`outer_ovl`** (not **`GtkPopover`**) — no separate compositor surface. **macOS:** opaque CSS on the frame at connect (`seek_bar_preview/macos_compositing.rs`); show/hide is **`set_visible` only**; theater reopen calls **`macos_shell_compositing::overlay_opened`**, every hide calls **`overlay_closed`** (same policy as header menus — no preview-specific raise/opacity/timers). See [`references-gtk4-macos-header-menus.md`](../references-gtk4-macos-header-menus.md) (**Theater overlay compositing**).
